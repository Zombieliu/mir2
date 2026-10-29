//! Ordered *interpretation*, not online authority or a durable world executor.
//!
//! Crystal NPCSegment.cs:3421-3467 gives new pets at the Human's exact cell,
//! kills matching pets in reverse list order, and merely marks CLEARPETS for
//! the next AI turn. HumanObject.cs:317-320 removes dead references later.
//! Checks therefore read the ordered Human.Pets list, including dead entries.
//! PETLEVEL deliberately follows the final-entry rule at NPCSegment.cs:2474-2485.
//!
//! The Root-owned Zone capture must authenticate the owner/life/map fence before
//! `from_verified_zone`. That name and this module's structural validation are
//! not authentication. Read sets and shadows cannot be deserialized. Persisted
//! plans are data only: there is deliberately no plan-to-live-read-set conversion.
//! Source CAS, receipt verification, object allocation, Spawned/Die/AI clocks,
//! owner-list cleanup, rollback and replay are separate integration work.

use super::zone::{OnlineOwner, ZoneKey};
use mir2_protocol::{MirDirection, Point};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

const PLAN_SCHEMA: u16 = 1;

/// Identity data in a plan, never a recovered online capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcPetPlanOwner {
    online: OnlineOwner,
    player_life_generation: u64,
    zone: ZoneKey,
    position: Point,
    direction: MirDirection,
}

impl NpcPetPlanOwner {
    pub(crate) fn online(&self) -> &OnlineOwner {
        &self.online
    }
    pub(crate) fn player_life_generation(&self) -> u64 {
        self.player_life_generation
    }
    pub(crate) fn zone(&self) -> &ZoneKey {
        &self.zone
    }
    pub(crate) fn position(&self) -> &Point {
        &self.position
    }
    pub(crate) fn direction(&self) -> MirDirection {
        self.direction
    }
}

/// A pre-existing life is resolved at interpretation, not by name on replay.
/// A birth ordinal is local to one plan and is not an online object identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum NpcPetTarget {
    Existing {
        zone: ZoneKey,
        object_id: u32,
        incarnation: u64,
    },
    NewBirth {
        ordinal: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcPetGrowth {
    pub(crate) level: u8,
    pub(crate) max_pet_level: u8,
    pub(crate) experience: u32,
}

/// Input from the trusted Zone capture of Human.Pets, in original list order.
/// In particular this must not be a filtered list of currently living pets or
/// all indirect monsters with the same owner_session_id (e.g. Totem children).
#[derive(Debug)]
pub(crate) struct NpcPetSourceRecord {
    pub(crate) zone: ZoneKey,
    pub(crate) object_id: u32,
    pub(crate) incarnation: u64,
    pub(crate) monster_index: i32,
    pub(crate) source_name: String,
    pub(crate) position: Point,
    pub(crate) experience_rate: NpcPetExperienceRate,
    pub(crate) growth: NpcPetGrowth,
    pub(crate) dead: bool,
    pub(crate) die_next_turn: bool,
}

/// The resolver/factory supplies canonical source info. Factory validation is
/// preparation only; neither callback may spawn an ECS or Zone object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpcPetTemplate {
    pub(crate) monster_index: i32,
    pub(crate) source_name: String,
    pub(crate) experience_rate: NpcPetExperienceRate,
}

/// Captured from canonical monster info and the active Source name settings by
/// the trusted producer. A serialized value cannot authenticate those settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NpcPetExperienceRate {
    Ordinary,
    ConfiguredTriple,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcPetPlannedRecord {
    target: NpcPetTarget,
    monster_index: i32,
    source_name: String,
    position: Point,
    experience_rate: NpcPetExperienceRate,
    growth: NpcPetGrowth,
    dead: bool,
    die_next_turn: bool,
}

impl NpcPetPlannedRecord {
    pub(crate) fn target(&self) -> &NpcPetTarget {
        &self.target
    }
    pub(crate) fn monster_index(&self) -> i32 {
        self.monster_index
    }
    pub(crate) fn source_name(&self) -> &str {
        &self.source_name
    }
    pub(crate) fn growth(&self) -> NpcPetGrowth {
        self.growth
    }
    pub(crate) fn position(&self) -> &Point {
        &self.position
    }
    pub(crate) fn experience_rate(&self) -> NpcPetExperienceRate {
        self.experience_rate
    }
    pub(crate) fn dead(&self) -> bool {
        self.dead
    }
    pub(crate) fn die_next_turn(&self) -> bool {
        self.die_next_turn
    }
}

/// One actual GainExp invocation's eligible-pet projection, not an aggregated
/// amount or an arbitrary level assignment. The trusted producer supplies the
/// Source amount projected for this pet (plain or the configured x3 names).
/// This module checks the captured map/range/name rate, uint wrapping and
/// at-most-one-level transition. The producer authenticates Source settings,
/// current positions, complete eligible membership and real GainExp admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcPetExperienceProjection {
    pub(crate) target: NpcPetTarget,
    pub(crate) before: NpcPetGrowth,
    pub(crate) after: NpcPetGrowth,
    pub(crate) applied_amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum NpcPetWorldOperation {
    GivePet {
        pet: NpcPetPlannedRecord,
        position: Point,
        direction: MirDirection,
    },
    /// Preserve the Human.Pets reference after real Die(), including an already
    /// dead target. The executor must retain real source death/corpse behavior.
    DiePet { target: NpcPetTarget },
    /// This does not invoke Die() now and is not a despawn instruction.
    MarkDieNextTurn { target: NpcPetTarget },
    PlayerExperienceStep {
        amount: u32,
        projections: Vec<NpcPetExperienceProjection>,
    },
}

/// A persistable interpretation. Deserialize proves neither a live owner nor a
/// committed Source CAS. Consumers must verify their separate trusted receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NpcPetWorldPlan {
    schema: u16,
    owner: NpcPetPlanOwner,
    before: Vec<NpcPetPlannedRecord>,
    operations: Vec<NpcPetWorldOperation>,
    after: Vec<NpcPetPlannedRecord>,
}

impl NpcPetWorldPlan {
    pub(crate) fn owner(&self) -> &NpcPetPlanOwner {
        &self.owner
    }
    pub(crate) fn before(&self) -> &[NpcPetPlannedRecord] {
        &self.before
    }
    pub(crate) fn operations(&self) -> &[NpcPetWorldOperation] {
        &self.operations
    }
    pub(crate) fn after(&self) -> &[NpcPetPlannedRecord] {
        &self.after
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Only checks self-consistency. Root must additionally check canonical
    /// source bindings, bounded save input and the authenticated durable receipt.
    /// This method never mints a read set or grants an online authority.
    pub(crate) fn validate_internal(&self) -> Result<(), NpcPetPlanError> {
        if self.schema != PLAN_SCHEMA {
            return Err(invalid_plan("unknown plan schema"));
        }
        validate_records(&self.before, true)?;
        let mut records = self.before.clone();
        let mut next_birth = 0_u32;
        for operation in &self.operations {
            match operation {
                NpcPetWorldOperation::GivePet {
                    pet,
                    position,
                    direction,
                } => {
                    if pet.target
                        != (NpcPetTarget::NewBirth {
                            ordinal: next_birth,
                        })
                        || pet.growth.level > 7
                        || pet.growth.max_pet_level != 7
                        || pet.growth.experience != 0
                        || pet.dead
                        || pet.die_next_turn
                        || position != &self.owner.position
                        || pet.position != self.owner.position
                        || *direction != self.owner.direction
                    {
                        return Err(invalid_plan(
                            "birth does not match its ordered source prefix",
                        ));
                    }
                    validate_record(pet)?;
                    next_birth = next_birth
                        .checked_add(1)
                        .ok_or(NpcPetPlanError::CounterExhausted)?;
                    records.push(pet.clone());
                }
                NpcPetWorldOperation::DiePet { target } => {
                    let record = record_mut(&mut records, target)?;
                    record.dead = true;
                }
                NpcPetWorldOperation::MarkDieNextTurn { target } => {
                    record_mut(&mut records, target)?.die_next_turn = true;
                }
                NpcPetWorldOperation::PlayerExperienceStep {
                    amount,
                    projections,
                } => {
                    if *amount == 0 {
                        return Err(invalid_plan("zero GainExp must not produce a step"));
                    }
                    validate_experience_step(&self.owner, &records, *amount, projections)?;
                    apply_experience_step(&mut records, projections)?;
                }
            }
        }
        if records != self.after {
            return Err(invalid_plan("plan after image differs from ordered replay"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct NpcPetReadSet {
    owner: NpcPetPlanOwner,
    pets: Vec<NpcPetPlannedRecord>,
}

impl NpcPetReadSet {
    /// Call only from a Root-owned fresh Zone capture after verifying owner,
    /// life, map and direct Human.Pets membership. Zero player life is valid.
    /// Structural checks cannot authenticate the provided OnlineOwner.
    pub(crate) fn from_verified_zone(
        owner: OnlineOwner,
        player_life_generation: u64,
        zone: ZoneKey,
        position: Point,
        direction: MirDirection,
        pets_in_source_order: Vec<NpcPetSourceRecord>,
    ) -> Result<Self, NpcPetPlanError> {
        let pets: Vec<_> = pets_in_source_order
            .into_iter()
            .map(|pet| NpcPetPlannedRecord {
                target: NpcPetTarget::Existing {
                    zone: pet.zone,
                    object_id: pet.object_id,
                    incarnation: pet.incarnation,
                },
                monster_index: pet.monster_index,
                source_name: pet.source_name,
                position: pet.position,
                experience_rate: pet.experience_rate,
                growth: pet.growth,
                dead: pet.dead,
                die_next_turn: pet.die_next_turn,
            })
            .collect();
        validate_records(&pets, true)?;
        Ok(Self {
            owner: NpcPetPlanOwner {
                online: owner,
                player_life_generation,
                zone,
                position,
                direction,
            },
            pets,
        })
    }
}

#[derive(Debug)]
pub(crate) struct NpcPetShadow {
    owner: NpcPetPlanOwner,
    before: Vec<NpcPetPlannedRecord>,
    pets: Vec<NpcPetPlannedRecord>,
    operations: Vec<NpcPetWorldOperation>,
    next_birth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcPetActionFlow {
    Continue,
    StopAct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NpcPetPlanError {
    InvalidPlan(String),
    InvalidComparisonOperator(String),
    InvalidExperienceProjection(String),
    CounterExhausted,
}

impl fmt::Display for NpcPetPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(message) => write!(formatter, "NPC pet plan: {message}"),
            Self::InvalidComparisonOperator(operator) => {
                write!(formatter, "invalid Source comparison operator: {operator}")
            }
            Self::InvalidExperienceProjection(message) => {
                write!(formatter, "NPC pet XP projection: {message}")
            }
            Self::CounterExhausted => formatter.write_str("NPC pet birth ordinal exhausted"),
        }
    }
}

impl std::error::Error for NpcPetPlanError {}

impl NpcPetShadow {
    pub(crate) fn new(read_set: NpcPetReadSet) -> Self {
        Self {
            owner: read_set.owner,
            before: read_set.pets.clone(),
            pets: read_set.pets,
            operations: Vec::new(),
            next_birth: 0,
        }
    }

    pub(crate) fn pets(&self) -> &[NpcPetPlannedRecord] {
        &self.pets
    }
    pub(crate) fn operations(&self) -> &[NpcPetWorldOperation] {
        &self.operations
    }

    /// NPCSegment.cs:533-539 inserts omitted "1"/"0" tokens; :3427-3432
    /// byte.TryParse uses count=1 / level=0 only for explicitly invalid tokens.
    /// Valid count 0 remains 0. Count is capped per invocation, not globally.
    /// Unknown info/factory failure StopAct preserves the already emitted prefix.
    pub(crate) fn give_pet<R, F>(
        &mut self,
        source_name: &str,
        count_token: Option<&str>,
        level_token: Option<&str>,
        mut resolve: R,
        mut instantiate: F,
    ) -> Result<NpcPetActionFlow, NpcPetPlanError>
    where
        R: FnMut(&str) -> Option<NpcPetTemplate>,
        F: FnMut(&NpcPetTemplate) -> Result<bool, NpcPetPlanError>,
    {
        let Some(template) = resolve(source_name) else {
            return Ok(NpcPetActionFlow::StopAct);
        };
        if template.source_name.is_empty()
            || template.monster_index < 0
            || !template.source_name.eq_ignore_ascii_case(source_name)
        {
            return Err(invalid_plan(
                "resolver did not return requested canonical source info",
            ));
        }
        let count = count_token
            .map_or(1, |token| source_byte(token).unwrap_or(1))
            .min(5);
        let level = level_token
            .map_or(0, |token| source_byte(token).unwrap_or(0))
            .min(7);
        let pet_len = self.pets.len();
        let operation_len = self.operations.len();
        let birth_before = self.next_birth;
        let outcome = (|| {
            for _ in 0..count {
                let ordinal = self.next_birth;
                let next = ordinal
                    .checked_add(1)
                    .ok_or(NpcPetPlanError::CounterExhausted)?;
                // Preparation only: the caller must not create real world objects.
                if !instantiate(&template)? {
                    return Ok(NpcPetActionFlow::StopAct);
                }
                let pet = NpcPetPlannedRecord {
                    target: NpcPetTarget::NewBirth { ordinal },
                    monster_index: template.monster_index,
                    source_name: template.source_name.clone(),
                    position: self.owner.position.clone(),
                    experience_rate: template.experience_rate,
                    growth: NpcPetGrowth {
                        level,
                        max_pet_level: 7,
                        experience: 0,
                    },
                    dead: false,
                    die_next_turn: false,
                };
                self.operations.push(NpcPetWorldOperation::GivePet {
                    pet: pet.clone(),
                    position: self.owner.position.clone(),
                    direction: self.owner.direction,
                });
                self.pets.push(pet);
                self.next_birth = next;
            }
            Ok(NpcPetActionFlow::Continue)
        })();
        if outcome.is_err() {
            // No external effects occurred. Discard this helper's preparation,
            // then the caller MUST reject/rollback the *whole* Source command,
            // including any fee/item action that preceded GIVEPET.
            self.pets.truncate(pet_len);
            self.operations.truncate(operation_len);
            self.next_birth = birth_before;
        }
        outcome
    }

    pub(crate) fn remove_pet(&mut self, source_name: &str) {
        for pet in self.pets.iter_mut().rev() {
            if !pet.source_name.eq_ignore_ascii_case(source_name) {
                continue;
            }
            self.operations.push(NpcPetWorldOperation::DiePet {
                target: pet.target.clone(),
            });
            pet.dead = true;
        }
    }

    pub(crate) fn clear_pets(&mut self) {
        for pet in self.pets.iter_mut().rev() {
            self.operations.push(NpcPetWorldOperation::MarkDieNextTurn {
                target: pet.target.clone(),
            });
            pet.die_next_turn = true;
        }
    }

    pub(crate) fn check_pet(&self, source_name: &str) -> bool {
        self.pets
            .iter()
            .rev()
            .any(|pet| pet.source_name.eq_ignore_ascii_case(source_name))
    }

    pub(crate) fn check_pet_count(
        &self,
        operator: &str,
        value: &str,
    ) -> Result<bool, NpcPetPlanError> {
        let Ok(value) = value.trim().parse::<i32>() else {
            return Ok(false);
        };
        let count = i32::try_from(self.pets.len())
            .map_err(|_| invalid_plan("pet list exceeds Source Int32 count"))?;
        source_compare(operator, count, value)
    }

    pub(crate) fn check_pet_level(
        &self,
        operator: &str,
        value: &str,
    ) -> Result<bool, NpcPetPlanError> {
        let Ok(value) = value.trim().parse::<i32>() else {
            return Ok(false);
        };
        // No Compare call occurs for an empty list, even for an invalid op.
        self.pets.last().map_or(Ok(true), |pet| {
            source_compare(operator, i32::from(pet.growth.level), value)
        })
    }

    /// Called once per actual successful player GainExp, in exact source order.
    /// Missing projections are legitimate ineligible pets, determined outside
    /// this interpreter by the trusted source map/range/name/class admission.
    pub(crate) fn record_player_experience_step(
        &mut self,
        amount: u32,
        projections: Vec<NpcPetExperienceProjection>,
    ) -> Result<(), NpcPetPlanError> {
        if amount == 0 {
            if !projections.is_empty() {
                return Err(invalid_xp("zero GainExp cannot update a pet"));
            }
            return Ok(());
        }
        validate_experience_step(&self.owner, &self.pets, amount, &projections)?;
        apply_experience_step(&mut self.pets, &projections)?;
        self.operations
            .push(NpcPetWorldOperation::PlayerExperienceStep {
                amount,
                projections,
            });
        Ok(())
    }

    pub(crate) fn into_plan(self) -> NpcPetWorldPlan {
        NpcPetWorldPlan {
            schema: PLAN_SCHEMA,
            owner: self.owner,
            before: self.before,
            operations: self.operations,
            after: self.pets,
        }
    }
}

fn invalid_plan(message: &str) -> NpcPetPlanError {
    NpcPetPlanError::InvalidPlan(message.into())
}
fn invalid_xp(message: &str) -> NpcPetPlanError {
    NpcPetPlanError::InvalidExperienceProjection(message.into())
}

fn validate_record(pet: &NpcPetPlannedRecord) -> Result<(), NpcPetPlanError> {
    if pet.monster_index < 0
        || pet.source_name.is_empty()
        || pet.growth.level > 7
        || pet.growth.max_pet_level > 7
    {
        return Err(invalid_plan(
            "pet record is outside source template/growth shape",
        ));
    }
    if !mir2_game_data::crystal_monster_by_index(pet.monster_index)
        .is_some_and(|source| source.name == pet.source_name)
    {
        return Err(invalid_plan(
            "pet record does not bind canonical source monster info",
        ));
    }
    Ok(())
}

fn validate_records(
    pets: &[NpcPetPlannedRecord],
    existing_only: bool,
) -> Result<(), NpcPetPlanError> {
    let mut seen = BTreeSet::new();
    for pet in pets {
        validate_record(pet)?;
        if existing_only && !matches!(pet.target, NpcPetTarget::Existing { .. }) {
            return Err(invalid_plan("source read set contains a plan-local birth"));
        }
        if !seen.insert(pet.target.clone()) {
            return Err(invalid_plan(
                "source list contains a duplicate life reference",
            ));
        }
    }
    Ok(())
}

fn record_mut<'a>(
    pets: &'a mut [NpcPetPlannedRecord],
    target: &NpcPetTarget,
) -> Result<&'a mut NpcPetPlannedRecord, NpcPetPlanError> {
    pets.iter_mut()
        .find(|pet| &pet.target == target)
        .ok_or_else(|| invalid_plan("operation references no current source-list pet"))
}

// .NET byte.TryParse with the source's Integer tokens: signed/whitespace
// decimal and negative zero are accepted, but overflow/negative/other text is
// invalid. We never reinterpret an invalid explicit token as an omitted one.
fn source_byte(token: &str) -> Option<u8> {
    let token = token.trim();
    let (negative, digits) = if let Some(rest) = token.strip_prefix('-') {
        (true, rest)
    } else {
        (false, token.strip_prefix('+').unwrap_or(token))
    };
    if digits.is_empty() {
        return None;
    }
    let mut value = 0_u16;
    for digit in digits.bytes() {
        if !digit.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(u16::from(digit - b'0'))?;
        if value > u16::from(u8::MAX) {
            return None;
        }
    }
    if negative && value != 0 {
        return None;
    }
    u8::try_from(value).ok()
}

// Crystal NPCSegment.cs:4996-5007. Unsupported aliases throw in source; do not
// quietly accept "=" or "<>" as invented operators.
fn source_compare(operator: &str, left: i32, right: i32) -> Result<bool, NpcPetPlanError> {
    match operator {
        "<" => Ok(left < right),
        ">" => Ok(left > right),
        "<=" => Ok(left <= right),
        ">=" => Ok(left >= right),
        "==" => Ok(left == right),
        "!=" => Ok(left != right),
        _ => Err(NpcPetPlanError::InvalidComparisonOperator(operator.into())),
    }
}

fn expected_pet_growth(before: NpcPetGrowth, applied_amount: u32) -> NpcPetGrowth {
    if before.level >= before.max_pet_level {
        return before;
    }
    // MonsterObject.cs:3502-3517: uint addition wraps; one call gains at most
    // one level. applied_amount already contains any source x3 wrapping.
    let mut after = before;
    after.experience = after.experience.wrapping_add(applied_amount);
    let threshold = (u32::from(before.level) + 1) * 20_000;
    if after.experience >= threshold {
        after.experience -= threshold;
        after.level += 1;
    }
    after
}

fn validate_experience_step(
    owner: &NpcPetPlanOwner,
    pets: &[NpcPetPlannedRecord],
    amount: u32,
    projections: &[NpcPetExperienceProjection],
) -> Result<(), NpcPetPlanError> {
    let mut seen = BTreeSet::new();
    let mut last_index = None;
    for projection in projections {
        if !seen.insert(projection.target.clone()) {
            return Err(invalid_xp("duplicate pet projection in one GainExp"));
        }
        let index = pets
            .iter()
            .position(|pet| pet.target == projection.target)
            .ok_or_else(|| invalid_xp("projection references no current source-list pet"))?;
        if last_index.is_some_and(|last| last >= index) {
            return Err(invalid_xp(
                "pet XP projections do not follow Source list order",
            ));
        }
        last_index = Some(index);
        let pet = &pets[index];
        if pet.dead || pet.growth != projection.before {
            return Err(invalid_xp("projection before image is stale or dead"));
        }
        if matches!(&pet.target, NpcPetTarget::Existing { zone, .. } if zone != &owner.zone)
            || pet.position.x.abs_diff(owner.position.x) > super::CRYSTAL_OBJECT_DATA_RANGE as u32
            || pet.position.y.abs_diff(owner.position.y) > super::CRYSTAL_OBJECT_DATA_RANGE as u32
        {
            return Err(invalid_xp(
                "Source pet is outside the owner's current map or DataRange",
            ));
        }
        let expected_amount = match pet.experience_rate {
            NpcPetExperienceRate::Ordinary => amount,
            NpcPetExperienceRate::ConfiguredTriple => amount.wrapping_mul(3),
        };
        if projection.applied_amount != expected_amount {
            return Err(invalid_xp(
                "applied pet amount differs from its captured Source name rate",
            ));
        }
        if projection.after != expected_pet_growth(projection.before, projection.applied_amount) {
            return Err(invalid_xp(
                "projection differs from single-call source PetExp",
            ));
        }
    }
    Ok(())
}

fn apply_experience_step(
    pets: &mut [NpcPetPlannedRecord],
    projections: &[NpcPetExperienceProjection],
) -> Result<(), NpcPetPlanError> {
    for projection in projections {
        record_mut(pets, &projection.target)?.growth = projection.after;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::zone::SessionId;

    // Pure isolated Source-shape fixtures: no account store, real saves,
    // privileged packets, world actors or production NPC binding are created.
    fn zone(map: &str) -> ZoneKey {
        ZoneKey {
            shard_id: "npc-plan-fixture".into(),
            map_file_name: map.into(),
            channel_id: 0,
            instance_id: "ordinary".into(),
        }
    }

    fn template(name: &str) -> Option<NpcPetTemplate> {
        mir2_game_data::crystal_monster_by_name(name).map(|source| NpcPetTemplate {
            monster_index: source.monster_index,
            source_name: source.name.clone(),
            // Default supplied Source settings; production capture must read
            // its own trusted current settings rather than reuse this fixture.
            experience_rate: if matches!(
                source.name.as_str(),
                "BoneFamiliar" | "Shinsu" | "HolyDeva"
            ) {
                NpcPetExperienceRate::ConfiguredTriple
            } else {
                NpcPetExperienceRate::Ordinary
            },
        })
    }

    fn pet(id: u32, name: &str, level: u8) -> NpcPetSourceRecord {
        let info = template(name).expect("fixture uses imported Crystal source info");
        NpcPetSourceRecord {
            zone: zone("0"),
            object_id: id,
            incarnation: u64::from(id) + 1,
            monster_index: info.monster_index,
            source_name: info.source_name,
            position: Point { x: 333, y: 259 },
            experience_rate: info.experience_rate,
            growth: NpcPetGrowth {
                level,
                max_pet_level: 7,
                experience: 0,
            },
            dead: false,
            die_next_turn: false,
        }
    }

    fn read_set(pets: Vec<NpcPetSourceRecord>) -> Result<NpcPetReadSet, NpcPetPlanError> {
        NpcPetReadSet::from_verified_zone(
            OnlineOwner {
                namespace: "isolated-source-fixture".into(),
                epoch: 1,
                session_id: SessionId::from("fixture-owner"),
                account_id: "fixture-account".into(),
                character_index: 7,
                object_id: 100,
            },
            0,
            zone("0"),
            Point { x: 333, y: 259 },
            MirDirection::DownLeft,
            pets,
        )
    }

    fn shadow(pets: Vec<NpcPetSourceRecord>) -> NpcPetShadow {
        NpcPetShadow::new(read_set(pets).expect("trusted isolated read set"))
    }

    fn give(
        shadow: &mut NpcPetShadow,
        count: Option<&str>,
        level: Option<&str>,
    ) -> NpcPetActionFlow {
        shadow
            .give_pet("RedViper", count, level, template, |_| Ok(true))
            .unwrap()
    }

    fn projection(pet: &NpcPetPlannedRecord, applied_amount: u32) -> NpcPetExperienceProjection {
        NpcPetExperienceProjection {
            target: pet.target.clone(),
            before: pet.growth,
            after: expected_pet_growth(pet.growth, applied_amount),
            applied_amount,
        }
    }

    #[test]
    fn omitted_parameters_give_one_zero_level_pet_at_exact_owner_cell() {
        let mut s = shadow(Vec::new());
        assert_eq!(give(&mut s, None, None), NpcPetActionFlow::Continue);
        let plan = s.into_plan();
        assert_eq!(plan.owner.player_life_generation, 0);
        assert_eq!(
            plan.after[0].growth,
            NpcPetGrowth {
                level: 0,
                max_pet_level: 7,
                experience: 0
            }
        );
        assert!(
            matches!(&plan.operations[0], NpcPetWorldOperation::GivePet { position, direction, .. }
            if *position == Point { x: 333, y: 259 } && *direction == MirDirection::DownLeft)
        );
        plan.validate_internal().unwrap();
    }

    #[test]
    fn explicit_zero_never_calls_factory_but_unknown_info_still_stops_act() {
        let mut s = shadow(Vec::new());
        let mut called = 0;
        assert_eq!(
            s.give_pet("RedViper", Some("0"), Some("7"), template, |_| {
                called += 1;
                Ok(true)
            })
            .unwrap(),
            NpcPetActionFlow::Continue
        );
        assert_eq!(called, 0);
        assert_eq!(
            s.give_pet(
                "not-a-source-template",
                Some("0"),
                None,
                |_| None,
                |_| panic!("unknown info must not call factory")
            )
            .unwrap(),
            NpcPetActionFlow::StopAct
        );
        assert!(s.into_plan().is_empty());
    }

    #[test]
    fn count_parser_preserves_zero_and_caps_each_valid_invocation_only() {
        for (token, expected) in [
            ("-1", 1),
            ("256", 1),
            ("1e1", 1),
            ("2_0", 1),
            ("+2", 2),
            ("255", 5),
            ("  +0005  ", 5),
            ("-0", 0),
            ("-000", 0),
            ("", 1),
        ] {
            let mut s = shadow(Vec::new());
            give(&mut s, Some(token), None);
            assert_eq!(s.pets.len(), expected, "Source count token {token:?}");
        }
    }

    #[test]
    fn byte_level_overflow_defaults_to_zero_and_valid_level_caps_at_seven() {
        for (token, expected) in [
            ("-1", 0),
            ("256", 0),
            ("bad", 0),
            ("255", 7),
            ("+2", 2),
            ("-0", 0),
        ] {
            let mut s = shadow(Vec::new());
            give(&mut s, None, Some(token));
            assert_eq!(
                s.pets[0].growth.level, expected,
                "Source level token {token:?}"
            );
        }
    }

    #[test]
    fn two_gives_do_not_cap_total_or_relevel_same_named_old_pets() {
        let mut s = shadow(vec![pet(21, "RedViper", 2), pet(22, "RedViper", 4)]);
        give(&mut s, Some("99"), Some("7"));
        give(&mut s, Some("5"), Some("1"));
        assert_eq!(s.pets.len(), 12);
        assert_eq!(
            s.pets.iter().map(|p| p.growth.level).collect::<Vec<_>>(),
            vec![2, 4, 7, 7, 7, 7, 7, 1, 1, 1, 1, 1]
        );
        assert!(s.operations.iter().all(
            |op| matches!(op, NpcPetWorldOperation::GivePet { position, .. }
            if *position == Point { x: 333, y: 259 })
        ));
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn source_factory_null_stops_remaining_act_and_keeps_birth_prefix() {
        let mut s = shadow(Vec::new());
        let mut calls = 0;
        let flow = s
            .give_pet("RedViper", Some("5"), Some("7"), template, |_| {
                calls += 1;
                Ok(calls < 3)
            })
            .unwrap();
        assert_eq!(flow, NpcPetActionFlow::StopAct);
        assert_eq!(calls, 3);
        assert_eq!(s.pets.len(), 2);
        let plan = s.into_plan();
        assert_eq!(plan.operations.len(), 2);
        plan.validate_internal().unwrap();
    }

    #[test]
    fn infrastructure_error_is_not_source_factory_null_and_discards_preparation() {
        let mut s = shadow(vec![pet(21, "RedViper", 2)]);
        let before = s.pets.clone();
        let mut calls = 0;
        let error = s
            .give_pet("RedViper", Some("5"), Some("7"), template, |_| {
                calls += 1;
                if calls == 3 {
                    Err(invalid_plan("isolated prepare failure"))
                } else {
                    Ok(true)
                }
            })
            .unwrap_err();
        assert!(matches!(error, NpcPetPlanError::InvalidPlan(_)));
        assert_eq!(s.pets, before);
        assert!(s.operations.is_empty());
        assert_eq!(s.next_birth, 0);
        // Root must also roll back preceding TAKEGOLD/TAKEITEM in its Source CAS
        // guard. This pure module has never modified either resource.
    }

    #[test]
    fn birth_counter_exhaustion_rejects_preparation_without_partial_append() {
        let mut s = shadow(Vec::new());
        s.next_birth = u32::MAX - 1;
        assert_eq!(
            s.give_pet("RedViper", Some("2"), None, template, |_| Ok(true)),
            Err(NpcPetPlanError::CounterExhausted)
        );
        assert!(s.pets.is_empty());
        assert!(s.operations.is_empty());
        assert_eq!(s.next_birth, u32::MAX - 1);
    }

    #[test]
    fn unknown_source_template_changes_no_existing_pet() {
        let mut s = shadow(vec![pet(21, "RedViper", 2)]);
        let before = s.pets.clone();
        assert_eq!(
            s.give_pet(
                "not-a-source-template",
                Some("5"),
                Some("7"),
                |_| None,
                |_| Ok(true)
            )
            .unwrap(),
            NpcPetActionFlow::StopAct
        );
        assert_eq!(s.pets, before);
        assert!(s.operations.is_empty());
    }

    #[test]
    fn noncanonical_resolver_result_is_an_error_not_a_substitute_monster() {
        let mut s = shadow(Vec::new());
        assert!(s
            .give_pet(
                "RedViper",
                None,
                None,
                |_| template("TigerSnake"),
                |_| Ok(true)
            )
            .is_err());
        assert!(s.operations.is_empty());
    }

    #[test]
    fn remove_binds_lives_in_reverse_order_without_removing_references() {
        let mut s = shadow(vec![
            pet(31, "RedViper", 2),
            pet(32, "TigerSnake", 4),
            pet(33, "RedViper", 7),
        ]);
        let first = s.pets[0].target.clone();
        let last = s.pets[2].target.clone();
        s.remove_pet("redviper");
        assert_eq!(
            s.operations,
            vec![
                NpcPetWorldOperation::DiePet { target: last },
                NpcPetWorldOperation::DiePet { target: first }
            ]
        );
        assert!(s.pets[0].dead && !s.pets[1].dead && s.pets[2].dead);
        assert!(s.check_pet("REDVIPER"));
        assert!(s.check_pet_count("==", "3").unwrap());
        assert!(s.check_pet_level("==", "7").unwrap());
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn repeated_remove_preserves_real_die_idempotence_and_source_list() {
        let mut dead = pet(31, "RedViper", 2);
        dead.dead = true;
        let mut s = shadow(vec![dead]);
        s.remove_pet("RedViper");
        s.remove_pet("RedViper");
        assert_eq!(s.pets.len(), 1);
        assert_eq!(s.operations.len(), 2);
        assert!(s.pets[0].dead);
        // The later real Zone Die helper must no-op when this life is dead;
        // these operations do not invent a second death, reward or drop.
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn clear_marks_next_ai_reverse_order_and_same_segment_checks_still_see_pets() {
        let mut s = shadow(vec![pet(31, "RedViper", 7), pet(32, "RedViper", 7)]);
        let first = s.pets[0].target.clone();
        let last = s.pets[1].target.clone();
        s.clear_pets();
        assert_eq!(
            s.operations,
            vec![
                NpcPetWorldOperation::MarkDieNextTurn { target: last },
                NpcPetWorldOperation::MarkDieNextTurn { target: first }
            ]
        );
        assert!(s.pets.iter().all(|p| p.die_next_turn && !p.dead));
        assert!(s.check_pet_count("==", "2").unwrap());
        assert!(s.check_pet("RedViper"));
        assert!(s.check_pet_level("==", "7").unwrap());
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn give_then_clear_then_give_has_no_retroactive_death_or_relevel() {
        let mut s = shadow(Vec::new());
        give(&mut s, Some("2"), Some("7"));
        s.clear_pets();
        give(&mut s, Some("1"), Some("0"));
        assert!(s.pets[0].die_next_turn && s.pets[1].die_next_turn);
        assert!(!s.pets[2].die_next_turn);
        assert_eq!(
            s.pets.iter().map(|p| p.growth.level).collect::<Vec<_>>(),
            vec![7, 7, 0]
        );
        assert!(!s.check_pet_level("==", "7").unwrap());
        assert!(s.check_pet_level("==", "0").unwrap());
        assert_eq!(s.pets[2].target, NpcPetTarget::NewBirth { ordinal: 2 });
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn remove_then_give_never_retargets_new_same_named_birth() {
        let mut s = shadow(vec![pet(31, "RedViper", 0)]);
        let old = s.pets[0].target.clone();
        s.remove_pet("RedViper");
        give(&mut s, None, Some("7"));
        assert_eq!(
            s.operations[0],
            NpcPetWorldOperation::DiePet { target: old }
        );
        assert!(s.pets[0].dead);
        assert!(!s.pets[1].dead);
        assert_eq!(s.pets[1].target, NpcPetTarget::NewBirth { ordinal: 0 });
    }

    #[test]
    fn petlevel_uses_final_reference_even_dead_and_empty_list_passes() {
        let mut s = shadow(vec![pet(31, "RedViper", 2), pet(32, "TigerSnake", 7)]);
        assert!(s.check_pet_level("==", "7").unwrap());
        s.remove_pet("TigerSnake");
        assert!(s.check_pet_level("==", "7").unwrap());
        assert!(shadow(Vec::new()).check_pet_level("==", "7").unwrap());
        assert!(shadow(Vec::new()).check_pet_level("invalid", "7").unwrap());
        assert!(!shadow(Vec::new())
            .check_pet_level("==", "not-an-int")
            .unwrap());
    }

    #[test]
    fn source_comparison_does_not_accept_invented_aliases() {
        let s = shadow(vec![pet(31, "RedViper", 2)]);
        assert!(matches!(
            s.check_pet_count("=", "1"),
            Err(NpcPetPlanError::InvalidComparisonOperator(_))
        ));
        assert!(matches!(
            s.check_pet_level("<>", "7"),
            Err(NpcPetPlanError::InvalidComparisonOperator(_))
        ));
        assert!(!s.check_pet_count("==", "2147483648").unwrap());
        assert!(s.check_pet_count("<=", "+1").unwrap());
    }

    #[test]
    fn source_read_set_rejects_duplicate_life_but_preserves_cross_map_references() {
        assert!(read_set(vec![pet(31, "RedViper", 0), pet(31, "RedViper", 0)]).is_err());
        let mut other = pet(32, "RedViper", 7);
        other.zone = zone("D001");
        let s = shadow(vec![pet(31, "RedViper", 0), other]);
        assert!(s.check_pet_count("==", "2").unwrap());
        assert!(
            matches!(&s.pets[1].target, NpcPetTarget::Existing { zone: key, .. } if key.map_file_name == "D001")
        );
    }

    #[test]
    fn source_pet_xp_rejects_foreign_map_without_losing_owner_list_reference() {
        let mut foreign = pet(32, "RedViper", 0);
        foreign.zone = zone("D001");
        let mut s = shadow(vec![foreign]);
        let before = s.pets.clone();
        let update = projection(&s.pets[0], 20_000);
        assert!(s
            .record_player_experience_step(20_000, vec![update])
            .is_err());
        assert_eq!(s.pets, before);
        assert!(s.check_pet_count("==", "1").unwrap());
        s.remove_pet("RedViper");
        assert!(s.pets[0].dead);
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn source_pet_xp_rejects_triple_amount_for_ordinary_red_viper() {
        let mut s = shadow(vec![pet(31, "RedViper", 0)]);
        let before = s.pets.clone();
        let update = projection(&s.pets[0], 30_000);
        assert!(s
            .record_player_experience_step(10_000, vec![update])
            .is_err());
        assert_eq!(s.pets, before);
        assert!(s.operations.is_empty());
    }

    #[test]
    fn source_pet_xp_accepts_data_range_edge_and_rejects_one_cell_beyond() {
        let mut edge = pet(31, "RedViper", 0);
        edge.position.x += super::super::CRYSTAL_OBJECT_DATA_RANGE;
        edge.position.y -= super::super::CRYSTAL_OBJECT_DATA_RANGE;
        let mut outside = pet(32, "RedViper", 0);
        outside.position.x += super::super::CRYSTAL_OBJECT_DATA_RANGE + 1;
        let mut s = shadow(vec![edge, outside]);
        let update = projection(&s.pets[0], 1);
        s.record_player_experience_step(1, vec![update]).unwrap();
        let before = s.pets.clone();
        let operations = s.operations.clone();
        let update = projection(&s.pets[1], 1);
        assert!(s.record_player_experience_step(1, vec![update]).is_err());
        assert_eq!(s.pets, before);
        assert_eq!(s.operations, operations);
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn persisted_xp_plan_rejects_foreign_map_replay_and_noncanonical_template() {
        let mut s = shadow(vec![pet(31, "RedViper", 0)]);
        let update = projection(&s.pets[0], 1);
        s.record_player_experience_step(1, vec![update]).unwrap();
        let plan = s.into_plan();
        plan.validate_internal().unwrap();
        let mut forged = plan.clone();
        forged.owner.zone = zone("D001");
        assert!(forged.validate_internal().is_err());
        let mut forged = plan;
        forged.before[0].source_name = "Shinsu".into();
        assert!(forged.validate_internal().is_err());
    }

    #[test]
    fn one_actual_experience_call_cannot_skip_multiple_pet_levels() {
        let mut s = shadow(Vec::new());
        give(&mut s, None, None);
        let update = projection(&s.pets[0], 100_000);
        assert_eq!(update.after.level, 1);
        assert_eq!(update.after.experience, 80_000);
        s.record_player_experience_step(100_000, vec![update])
            .unwrap();
        assert!(s.check_pet_level("==", "1").unwrap());
        let second = projection(&s.pets[0], 1);
        s.record_player_experience_step(1, vec![second]).unwrap();
        assert_eq!(s.pets[0].growth.level, 2);
        assert_eq!(s.pets[0].growth.experience, 40_001);
        assert!(matches!(
            &s.operations[1],
            NpcPetWorldOperation::PlayerExperienceStep {
                amount: 100_000,
                ..
            }
        ));
        assert!(matches!(
            &s.operations[2],
            NpcPetWorldOperation::PlayerExperienceStep { amount: 1, .. }
        ));
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn max_level_pet_projection_is_unchanged_not_reset_or_relevelled() {
        let mut s = shadow(vec![pet(31, "RedViper", 7)]);
        s.pets[0].growth.experience = 123;
        s.before[0].growth.experience = 123;
        let update = projection(&s.pets[0], 100_000);
        assert_eq!(update.before, update.after);
        s.record_player_experience_step(100_000, vec![update])
            .unwrap();
        assert_eq!(s.pets[0].growth.experience, 123);
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn source_pet_xp_uint_addition_and_x3_amount_wrap() {
        let mut p = pet(31, "RedViper", 0);
        p.growth.experience = u32::MAX - 5;
        let mut s = shadow(vec![p]);
        let update = projection(&s.pets[0], 10);
        assert_eq!(update.after.experience, 4);
        assert_eq!(update.after.level, 0);
        s.record_player_experience_step(10, vec![update]).unwrap();
        s.into_plan().validate_internal().unwrap();
        let mut named = pet(32, "Shinsu", 0);
        named.growth.experience = 4;
        let mut s = shadow(vec![named]);
        let amount: u32 = 0x5555_5557;
        let applied = amount.wrapping_mul(3);
        assert_eq!(applied, 5);
        let update = projection(&s.pets[0], applied);
        s.record_player_experience_step(amount, vec![update])
            .unwrap();
        assert_eq!(s.pets[0].growth.experience, 9);
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn marked_next_turn_pet_is_still_eligible_until_actual_death() {
        let mut s = shadow(vec![pet(31, "RedViper", 0)]);
        s.clear_pets();
        let update = projection(&s.pets[0], 20_000);
        s.record_player_experience_step(20_000, vec![update])
            .unwrap();
        assert_eq!(s.pets[0].growth.level, 1);
        assert!(s.pets[0].die_next_turn && !s.pets[0].dead);
        s.remove_pet("RedViper");
        let update = projection(&s.pets[0], 40_000);
        let before = s.pets.clone();
        assert!(s
            .record_player_experience_step(40_000, vec![update])
            .is_err());
        assert_eq!(s.pets, before);
    }

    #[test]
    fn invalid_xp_projection_rejects_stale_arbitrary_level_and_amount() {
        let mut s = shadow(vec![pet(31, "RedViper", 0)]);
        let before = s.pets.clone();
        let mut update = projection(&s.pets[0], 20_000);
        update.after.level = 7;
        assert!(s
            .record_player_experience_step(20_000, vec![update])
            .is_err());
        let mut update = projection(&s.pets[0], 20_000);
        update.before.experience = 1;
        assert!(s
            .record_player_experience_step(20_000, vec![update])
            .is_err());
        let update = projection(&s.pets[0], 21_000);
        assert!(s
            .record_player_experience_step(20_000, vec![update])
            .is_err());
        assert_eq!(s.pets, before);
        assert!(s.operations.is_empty());
    }

    #[test]
    fn xp_projections_keep_source_list_order_and_cannot_duplicate_targets() {
        let mut s = shadow(vec![pet(31, "RedViper", 0), pet(32, "RedViper", 0)]);
        let a = projection(&s.pets[0], 20_000);
        let b = projection(&s.pets[1], 20_000);
        assert!(s
            .record_player_experience_step(20_000, vec![b.clone(), a.clone()])
            .is_err());
        assert!(s
            .record_player_experience_step(20_000, vec![a.clone(), a.clone()])
            .is_err());
        assert!(s.operations.is_empty());
        s.record_player_experience_step(20_000, vec![a, b]).unwrap();
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn zero_actual_gain_is_a_noop_and_no_eligible_pets_still_records_actual_call() {
        let mut s = shadow(Vec::new());
        s.record_player_experience_step(0, Vec::new()).unwrap();
        assert!(s.operations.is_empty());
        s.record_player_experience_step(12, Vec::new()).unwrap();
        assert_eq!(
            s.operations,
            vec![NpcPetWorldOperation::PlayerExperienceStep {
                amount: 12,
                projections: Vec::new()
            }]
        );
        s.into_plan().validate_internal().unwrap();
    }

    #[test]
    fn plan_roundtrip_is_only_data_and_ordered_replay_detects_forged_after_image() {
        let mut s = shadow(vec![pet(31, "RedViper", 0)]);
        give(&mut s, None, Some("7"));
        s.remove_pet("RedViper");
        let plan = s.into_plan();
        let bytes = serde_json::to_vec(&plan).unwrap();
        let decoded: NpcPetWorldPlan = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, plan);
        decoded.validate_internal().unwrap();
        let mut forged = decoded;
        forged.after[0].dead = false;
        assert!(forged.validate_internal().is_err());
        // There is intentionally no conversion from this decoded data to a
        // read set, fresh Source admission, live object or committed receipt.
    }

    #[test]
    fn plan_rejects_nonprefix_birth_unknown_target_offset_and_unknown_schema() {
        let mut s = shadow(Vec::new());
        give(&mut s, None, None);
        let plan = s.into_plan();
        let mut wrong = plan.clone();
        if let NpcPetWorldOperation::GivePet { pet, .. } = &mut wrong.operations[0] {
            pet.target = NpcPetTarget::NewBirth { ordinal: 7 };
        }
        assert!(wrong.validate_internal().is_err());
        let mut wrong = plan.clone();
        wrong.operations.push(NpcPetWorldOperation::DiePet {
            target: NpcPetTarget::NewBirth { ordinal: 7 },
        });
        assert!(wrong.validate_internal().is_err());
        let mut wrong = plan.clone();
        if let NpcPetWorldOperation::GivePet { position, .. } = &mut wrong.operations[0] {
            position.x += 1;
        }
        assert!(wrong.validate_internal().is_err());
        let mut wrong = plan;
        wrong.schema = 99;
        assert!(wrong.validate_internal().is_err());
    }

    #[test]
    fn serde_unknown_fields_cannot_be_smuggled_into_plan_or_operation() {
        let mut s = shadow(Vec::new());
        give(&mut s, None, None);
        let plan = s.into_plan();
        let mut value = serde_json::to_value(&plan).unwrap();
        value["online_authorized"] = serde_json::json!(true);
        assert!(serde_json::from_value::<NpcPetWorldPlan>(value).is_err());
        let mut value = serde_json::to_value(&plan).unwrap();
        value["operations"][0]["online_authorized"] = serde_json::json!(true);
        assert!(serde_json::from_value::<NpcPetWorldPlan>(value).is_err());
    }
}
