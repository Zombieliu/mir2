//! Exclusive full-owner application for the material protocol. A server
//! receipt, successful predecode or producer admission is never Applied.
//!
//! The physical reader first proves its current opaque client candidate, then
//! prepares this indivisible bundle from that same owner turn. NativeInbound
//! admits it into the existing protected FIFO, accounts its bytes and retires
//! queued gates on reset. Only the single owner-barrier consumer applies it
//! after the ordinary typed consumers have finished the entire FIFO prefix.

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, Weak};

use bevy::prelude::{Resource, World};
use mir2_client_wire::hunting_material::{Quote, Receipt};
use serde_json::Value;

use crate::npc_purchase_economy::{
    commit_complete_owner_models, preserve_repair_hero_presentation,
    retire_purchase_source_for_repair, Decoded, NativeHeroOwnerEpoch, NativeHeroOwnerGate,
    NativeNpcEconomyError, NativeNpcEconomyProjection,
};

const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;
const MAX_APPLIED: usize = 32;
const MAX_CUSTODY_NODES: usize = 4096;
const MAX_SOCKET_DEPTH: usize = 32;

pub type NativeHuntingMaterialError = NativeNpcEconomyError;

/// The actual host NativeCommandStamp, never a server save revision. Map 0 is
/// valid; an unselected actor cannot construct a material owner lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeHuntingMaterialEpoch {
    pub run: u64,
    pub connection: u64,
    pub procedure: u64,
    pub owner_epoch: u64,
    pub scene_epoch: u64,
    pub cancellation: u64,
    pub actor: u32,
    pub map: i32,
}

impl NativeHuntingMaterialEpoch {
    fn valid(self) -> bool {
        self.run != 0
            && self.connection != 0
            && self.actor != 0
            && self.map >= 0
            && ![
                self.run,
                self.connection,
                self.procedure,
                self.owner_epoch,
                self.scene_epoch,
                self.cancellation,
            ]
            .contains(&u64::MAX)
    }

    fn hero_epoch(self) -> NativeHeroOwnerEpoch {
        NativeHeroOwnerEpoch {
            run: self.run,
            connection: self.connection,
            procedure: self.procedure,
            owner_epoch: self.owner_epoch,
            scene_epoch: self.scene_epoch,
            cancellation: self.cancellation,
            actor: self.actor,
            map: self.map,
        }
    }
}

/// Learned only from the verified physical connection's selected character.
/// This is local custody, not a bearer token or a client-minted server owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeHuntingMaterialOwner {
    realm: String,
    account: String,
    index: i32,
}

impl NativeHuntingMaterialOwner {
    pub fn new(realm: &str, account: &str, index: i32) -> Result<Self, NativeHuntingMaterialError> {
        fn text(value: &str, limit: usize) -> bool {
            !value.is_empty()
                && value.len() <= limit
                && value.trim() == value
                && !value.chars().any(char::is_control)
        }
        if !text(realm, 128) || !text(account, 160) || index < 0 {
            return Err(NativeNpcEconomyError::Correlation);
        }
        Ok(Self {
            realm: realm.into(),
            account: account.into(),
            index,
        })
    }
    pub fn realm(&self) -> &str {
        &self.realm
    }
    pub fn account(&self) -> &str {
        &self.account
    }
    pub fn character_index(&self) -> i32 {
        self.index
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Application {
    request_id: u64,
    // Checked local gate revision. No server revision is invented from a tick,
    // receipt timestamp, inventory counter or this preparation sequence.
    revision: u64,
    quote: Arc<Quote>,
    receipt: Arc<Receipt>,
}

impl Application {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(std::mem::size_of::<Quote>())
            .saturating_add(std::mem::size_of::<Receipt>())
            .saturating_add(self.quote.quote_id.capacity())
            .saturating_add(self.quote.valuation_version.capacity())
            .saturating_add(self.receipt.quote_id.capacity())
            .saturating_add(self.receipt.valuation_version.capacity())
            .saturating_add(self.receipt.operation_key.capacity())
            .saturating_add(self.receipt.request_digest.capacity())
    }
}

#[derive(Debug)]
struct Custody {
    active: bool,
    epoch: NativeHuntingMaterialEpoch,
    owner: NativeHuntingMaterialOwner,
    prepared: Option<Application>,
    applied: Option<Application>,
    completions: VecDeque<NativeHuntingMaterialApplied>,
}

/// Independent material gate. The physical owner retires it before connection,
/// actor, scene, context withdrawal or protocol loss. Reset admission must also
/// retire queued gates, including a bundle that has not reached World yet.
#[derive(Debug, Clone)]
pub struct NativeHuntingMaterialGate {
    custody: Arc<Mutex<Custody>>,
}

impl NativeHuntingMaterialGate {
    pub fn new(
        epoch: NativeHuntingMaterialEpoch,
        owner: NativeHuntingMaterialOwner,
    ) -> Result<Self, NativeHuntingMaterialError> {
        if !epoch.valid() {
            return Err(NativeNpcEconomyError::Correlation);
        }
        Ok(Self {
            custody: Arc::new(Mutex::new(Custody {
                active: true,
                epoch,
                owner,
                prepared: None,
                applied: None,
                completions: VecDeque::new(),
            })),
        })
    }

    pub fn epoch(&self) -> Option<NativeHuntingMaterialEpoch> {
        self.custody
            .lock()
            .ok()
            .filter(|state| state.active)
            .map(|state| state.epoch)
    }

    pub fn owner(&self) -> Option<NativeHuntingMaterialOwner> {
        self.custody
            .lock()
            .ok()
            .filter(|state| state.active)
            .map(|state| state.owner.clone())
    }

    pub fn retire(&self) {
        if let Ok(mut state) = self.custody.lock() {
            state.active = false;
            state.completions.clear();
        }
    }

    pub fn is_current(
        &self,
        epoch: NativeHuntingMaterialEpoch,
        owner: &NativeHuntingMaterialOwner,
    ) -> bool {
        self.custody
            .lock()
            .is_ok_and(|state| state.active && state.epoch == epoch && state.owner == *owner)
    }

    /// The caller has already correlated the strict Committed frame and its
    /// opaque Windows candidate under the actual physical-owner fence. This
    /// performs full predecode and preserves the original typed quote/receipt;
    /// it does not enqueue, publish an inventory source or acknowledge success.
    pub fn prepare(
        &self,
        request_id: u64,
        quote: &Quote,
        receipt: &Receipt,
        projection: NativeNpcEconomyProjection,
    ) -> Result<NativeHuntingMaterialBundle, NativeHuntingMaterialError> {
        if request_id == 0 || request_id == u64::MAX || receipt.validate_for_quote(quote).is_err() {
            return Err(NativeNpcEconomyError::Correlation);
        }
        if projection.retained_bytes() > MAX_BUNDLE_BYTES {
            return Err(NativeNpcEconomyError::TooLarge);
        }
        let decoded = Decoded::decode(&projection)?;
        let quote = Arc::new(quote.clone());
        let receipt = Arc::new(receipt.clone());
        let mut state = self
            .custody
            .lock()
            .map_err(|_| NativeNpcEconomyError::Retired)?;
        if !state.active {
            return Err(NativeNpcEconomyError::Retired);
        }
        if state
            .prepared
            .as_ref()
            .is_some_and(|last| request_id <= last.request_id)
        {
            return Err(NativeNpcEconomyError::Correlation);
        }
        validate_material_owner(&decoded.owner, state.epoch, &quote, &receipt)?;
        let revision = state
            .prepared
            .as_ref()
            .map_or(0, |last| last.revision)
            .checked_add(1)
            .filter(|revision| *revision != u64::MAX);
        let Some(revision) = revision else {
            state.active = false;
            state.completions.clear();
            return Err(NativeNpcEconomyError::Retired);
        };
        let application = Application {
            request_id,
            revision,
            quote,
            receipt,
        };
        let bundle = NativeHuntingMaterialBundle {
            gate: self.clone(),
            epoch: state.epoch,
            owner: state.owner.clone(),
            application: application.clone(),
            projection: Arc::new(projection),
            hero_owner_gate: None,
        };
        if bundle.retained_bytes() > MAX_BUNDLE_BYTES {
            return Err(NativeNpcEconomyError::TooLarge);
        }
        state.prepared = Some(application);
        Ok(bundle)
    }

    pub fn try_recv_applied(&self) -> Option<NativeHuntingMaterialApplied> {
        let mut state = self.custody.lock().ok()?;
        if !state.active {
            return None;
        }
        state.completions.pop_front()
    }

    /// The physical fence must be held outside this final callback, and the
    /// Windows candidate must still be current there. This lock prevents reset
    /// retirement between reauthorization of the opaque proof and Complete.
    /// The callback must not re-enter this same gate or its World commit.
    /// It must not enqueue NativeInbound messages either: queue admission uses
    /// buffer -> custody lock order. Publish the cloned display model after
    /// this callback returns, while the outer physical-owner fence is held.
    pub fn with_current_applied<T>(
        &self,
        applied: &NativeHuntingMaterialApplied,
        complete: impl FnOnce() -> T,
    ) -> Option<T> {
        if !Weak::ptr_eq(&Arc::downgrade(&self.custody), &applied.gate) {
            return None;
        }
        let state = self.custody.lock().ok()?;
        if !state.active
            || state.epoch != applied.epoch
            || state.owner != applied.owner
            || state.prepared.as_ref() != Some(&applied.application)
            || state.applied.as_ref() != Some(&applied.application)
        {
            return None;
        }
        let completed = complete();
        drop(state);
        Some(completed)
    }

    pub(crate) fn same_gate(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.custody, &other.custody)
    }
}

#[derive(Debug, Clone)]
pub struct NativeHuntingMaterialBundle {
    gate: NativeHuntingMaterialGate,
    epoch: NativeHuntingMaterialEpoch,
    owner: NativeHuntingMaterialOwner,
    application: Application,
    projection: Arc<NativeNpcEconomyProjection>,
    hero_owner_gate: Option<NativeHeroOwnerGate>,
}

impl NativeHuntingMaterialBundle {
    pub fn with_hero_owner_gate(
        mut self,
        gate: &NativeHeroOwnerGate,
    ) -> Result<Self, NativeHuntingMaterialError> {
        if gate.epoch() != self.epoch.hero_epoch() {
            return Err(NativeNpcEconomyError::Correlation);
        }
        if gate.with_active(|| ()).is_none() || !self.is_current() {
            return Err(NativeNpcEconomyError::Retired);
        }
        self.hero_owner_gate = Some(gate.clone());
        if self.retained_bytes() > MAX_BUNDLE_BYTES {
            return Err(NativeNpcEconomyError::TooLarge);
        }
        Ok(self)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.projection.retained_bytes())
            .saturating_add(self.application.retained_bytes())
            .saturating_add(self.owner.realm.capacity())
            .saturating_add(self.owner.account.capacity())
            .saturating_add(
                self.hero_owner_gate
                    .as_ref()
                    .map_or(0, |_| std::mem::size_of::<NativeHeroOwnerGate>()),
            )
    }
    pub(crate) fn gate(&self) -> &NativeHuntingMaterialGate {
        &self.gate
    }
    pub(crate) fn is_current(&self) -> bool {
        self.gate.custody.lock().is_ok_and(|state| {
            state.active
                && state.epoch == self.epoch
                && state.owner == self.owner
                && state.prepared.as_ref() == Some(&self.application)
        })
    }
    pub(crate) fn retire(&self) {
        self.gate.retire();
    }
}

/// No public constructor, mutable fields or Deserialize. Only the real full
/// World commit below creates this proof. It binds the original gate instance,
/// physical actor/owner, request, typed receipt and local application revision.
#[derive(Debug, Clone)]
pub struct NativeHuntingMaterialApplied {
    // Weak avoids a custody -> queued completion -> custody reference cycle.
    gate: Weak<Mutex<Custody>>,
    epoch: NativeHuntingMaterialEpoch,
    owner: NativeHuntingMaterialOwner,
    application: Application,
}

impl NativeHuntingMaterialApplied {
    pub fn epoch(&self) -> NativeHuntingMaterialEpoch {
        self.epoch
    }
    pub fn owner(&self) -> &NativeHuntingMaterialOwner {
        &self.owner
    }
    pub fn request_id(&self) -> u64 {
        self.application.request_id
    }
    pub fn quote_id(&self) -> &str {
        &self.application.quote.quote_id
    }
    pub fn quote(&self) -> &Quote {
        &self.application.quote
    }
    pub fn receipt(&self) -> &Receipt {
        &self.application.receipt
    }
    pub fn application_revision(&self) -> u64 {
        self.application.revision
    }
}

impl PartialEq for NativeHuntingMaterialApplied {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.gate, &other.gate)
            && self.epoch == other.epoch
            && self.owner == other.owner
            && self.application == other.application
    }
}
impl Eq for NativeHuntingMaterialApplied {}

#[derive(Debug, Clone, Resource)]
pub struct NativeHuntingMaterialSource {
    applied: NativeHuntingMaterialApplied,
    projection: Arc<NativeNpcEconomyProjection>,
    owner: Arc<Value>,
    gate: NativeHuntingMaterialGate,
    reset_revision: u64,
    scene_revision: u64,
}

impl NativeHuntingMaterialSource {
    pub fn owner(&self) -> &Value {
        &self.owner
    }
    pub fn owner_json(&self) -> &str {
        &self.projection.owner_json
    }
    pub fn applied(&self) -> &NativeHuntingMaterialApplied {
        &self.applied
    }
}

pub(crate) fn clear_native_hunting_material_source(world: &mut World) {
    if let Some(source) = world.remove_resource::<NativeHuntingMaterialSource>() {
        source.gate.retire();
    }
}

pub(crate) fn clear_stale_native_hunting_material_source(world: &mut World) {
    let reset = world
        .get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>()
        .map_or(0, |r| r.0);
    let scene = world
        .get_resource::<crate::SceneResetRevision>()
        .map_or(0, |r| r.0);
    if world
        .get_resource::<NativeHuntingMaterialSource>()
        .is_some_and(|source| source.reset_revision != reset || source.scene_revision != scene)
    {
        clear_native_hunting_material_source(world);
    }
}

/// Tail of the single purchase -> repair -> material owner-barrier consumer.
/// This is not separately registered as a competing Bevy system. Prefix typed
/// consumers must have drained before the material bundle becomes queue front.
pub(crate) fn apply_pending_native_hunting_material(world: &mut World) {
    clear_stale_native_hunting_material_source(world);
    let bundle = world
        .get_resource::<crate::native_ingest::NativeInbound>()
        .and_then(|native| native.take_hunting_material_front());
    if let Some(bundle) = bundle {
        let _ = apply_native_hunting_material_bundle(world, bundle);
    }
}

/// Called only by the existing exclusive owner-barrier consumer after dequeue.
/// This returns no caller-assertable Applied flag. Even successful application
/// requires the opaque completion and current physical fence before UI Complete.
pub fn apply_native_hunting_material_bundle(
    world: &mut World,
    bundle: NativeHuntingMaterialBundle,
) -> Result<(), NativeHuntingMaterialError> {
    clear_stale_native_hunting_material_source(world);
    let mut decoded = Decoded::decode(&bundle.projection)?;
    validate_material_owner(
        &decoded.owner,
        bundle.epoch,
        &bundle.application.quote,
        &bundle.application.receipt,
    )?;
    let mut custody = bundle
        .gate
        .custody
        .lock()
        .map_err(|_| NativeNpcEconomyError::Retired)?;
    if !custody.active
        || custody.epoch != bundle.epoch
        || custody.owner != bundle.owner
        || custody.prepared.as_ref() != Some(&bundle.application)
        || custody
            .applied
            .as_ref()
            .is_some_and(|last| last.revision >= bundle.application.revision)
        || custody.completions.len() >= MAX_APPLIED
    {
        return Err(NativeNpcEconomyError::Retired);
    }
    let display = bundle.hero_owner_gate.clone();
    let commit = || {
        preserve_repair_hero_presentation(world, &mut decoded, display.as_ref());
        retire_purchase_source_for_repair(world);
        crate::hunting_repair_economy::clear_native_hunting_repair_source(world);
        if let Some(prior) = world.remove_resource::<NativeHuntingMaterialSource>() {
            if !prior.gate.same_gate(&bundle.gate) {
                prior.gate.retire();
            }
        }
        let owner = commit_complete_owner_models(world, decoded, display.clone());
        let applied = NativeHuntingMaterialApplied {
            gate: Arc::downgrade(&bundle.gate.custody),
            epoch: bundle.epoch,
            owner: bundle.owner.clone(),
            application: bundle.application.clone(),
        };
        let reset_revision = world
            .get_resource::<mir2_client_bevy::pending_operations::SessionResetRevision>()
            .map_or(0, |r| r.0);
        let scene_revision = world
            .get_resource::<crate::SceneResetRevision>()
            .map_or(0, |r| r.0);
        world.insert_resource(NativeHuntingMaterialSource {
            applied: applied.clone(),
            projection: Arc::clone(&bundle.projection),
            owner: Arc::new(owner),
            gate: bundle.gate.clone(),
            reset_revision,
            scene_revision,
        });
        custody.applied = Some(bundle.application.clone());
        custody.completions.push_back(applied);
    };
    if let Some(gate) = display.as_ref() {
        gate.with_active(commit)
            .ok_or(NativeNpcEconomyError::Retired)?;
    } else {
        commit();
    }
    Ok(())
}

fn unsigned(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        let s = value.as_str()?;
        if s.is_empty()
            || s != "0" && (s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        s.parse().ok()
    })
}

fn field_unsigned(value: &Value, key: &str) -> Result<u64, NativeHuntingMaterialError> {
    value
        .get(key)
        .and_then(unsigned)
        .ok_or(NativeNpcEconomyError::Projection)
}

fn bag_cell(item: &Value, capacity: u64) -> Result<(u64, u64), NativeHuntingMaterialError> {
    let slot = field_unsigned(item, "slot")?;
    let base = match item.get("container").and_then(Value::as_str) {
        Some("bag1") => 0,
        Some("bag2") => 40,
        _ => return Err(NativeNpcEconomyError::Projection),
    };
    if slot >= 40 || base + slot >= capacity {
        return Err(NativeNpcEconomyError::Projection);
    }
    Ok((base, slot))
}

fn validate_material_owner(
    owner: &Value,
    epoch: NativeHuntingMaterialEpoch,
    quote: &Quote,
    receipt: &Receipt,
) -> Result<(), NativeHuntingMaterialError> {
    if receipt.validate_for_quote(quote).is_err()
        || field_unsigned(owner, "playerObjectId")? != u64::from(epoch.actor)
        || owner.get("mapIndex").and_then(Value::as_i64) != Some(i64::from(epoch.map))
    {
        return Err(NativeNpcEconomyError::Correlation);
    }
    let capacity = field_unsigned(owner, "inventoryCapacity")?
        .checked_sub(6)
        .ok_or(NativeNpcEconomyError::Projection)?;
    let bag = owner
        .get("inventoryItems")
        .and_then(Value::as_array)
        .ok_or(NativeNpcEconomyError::Incomplete)?;
    let mut cells = BTreeSet::new();
    let mut quest_cells = BTreeSet::new();
    for item in bag {
        // Quest roots share inventoryItems with ordinary Bag roots, but use
        // their own fixed 80-cell read-only grouping. They are current owner
        // custody, never authority to select a material conversion source.
        if item.get("container").and_then(Value::as_str) == Some("quest") {
            let slot = field_unsigned(item, "slot")?;
            if slot >= 80 || !quest_cells.insert(slot) {
                return Err(NativeNpcEconomyError::Projection);
            }
        } else if !cells.insert(bag_cell(item, capacity)?) {
            return Err(NativeNpcEconomyError::Projection);
        }
    }
    // A readonly Query carries the original immutable receipt and CURRENT
    // complete owner state. Its output may since have been used, split, moved
    // or traded. Historical receipt quantities are never current eligibility.
    // Known original identities still cannot be ambiguous in this snapshot.
    let mut output_count = 0usize;
    let mut input_count = 0usize;
    let mut nodes = 0usize;
    for key in [
        "inventoryItems",
        "beltItems",
        "equipmentItems",
        "storageItems",
        "heroInventoryItems",
        "heroEquipmentItems",
    ] {
        let rows = owner
            .get(key)
            .and_then(Value::as_array)
            .ok_or(NativeNpcEconomyError::Incomplete)?;
        if rows.len().saturating_add(nodes) > MAX_CUSTODY_NODES {
            return Err(NativeNpcEconomyError::TooLarge);
        }
        for item in rows {
            nodes += 1;
            let uid = match item.get("uniqueId").filter(|uid| !uid.is_null()) {
                Some(uid) => Some(unsigned(uid).ok_or(NativeNpcEconomyError::Projection)?),
                None if matches!(key, "equipmentItems" | "heroEquipmentItems") => None,
                None => return Err(NativeNpcEconomyError::Projection),
            };
            output_count += usize::from(uid == Some(receipt.output_unique_id.get()));
            input_count += usize::from(uid == Some(receipt.input_unique_id.get()));
            if let Some(user) = item
                .get("tooltipSource")
                .and_then(|tooltip| tooltip.get("userItem"))
                .filter(|v| !v.is_null())
            {
                // Legacy equipment may have no retained outer UID. Preserve
                // that unknown state; use an actual wire UID when present,
                // and never invent one from its slot or display name.
                let wire_uid = field_unsigned(user, "unique_id")?;
                if uid.is_some_and(|uid| uid != wire_uid) {
                    return Err(NativeNpcEconomyError::Projection);
                }
                if uid.is_none() {
                    output_count += usize::from(wire_uid == receipt.output_unique_id.get());
                    input_count += usize::from(wire_uid == receipt.input_unique_id.get());
                }
                count_socket_uids(
                    user,
                    0,
                    &mut nodes,
                    receipt,
                    &mut output_count,
                    &mut input_count,
                )?;
            }
        }
    }
    if output_count > 1 || input_count > 1 {
        return Err(NativeNpcEconomyError::Projection);
    }
    Ok(())
}

fn count_socket_uids(
    user: &Value,
    depth: usize,
    nodes: &mut usize,
    receipt: &Receipt,
    outputs: &mut usize,
    inputs: &mut usize,
) -> Result<(), NativeHuntingMaterialError> {
    if depth > MAX_SOCKET_DEPTH {
        return Err(NativeNpcEconomyError::TooLarge);
    }
    let slots = user
        .get("slots")
        .and_then(Value::as_array)
        .ok_or(NativeNpcEconomyError::Incomplete)?;
    if slots.len().saturating_add(*nodes) > MAX_CUSTODY_NODES {
        return Err(NativeNpcEconomyError::TooLarge);
    }
    for child in slots.iter().filter(|child| !child.is_null()) {
        *nodes += 1;
        let uid = field_unsigned(child, "unique_id")?;
        *outputs += usize::from(uid == receipt.output_unique_id.get());
        *inputs += usize::from(uid == receipt.input_unique_id.get());
        count_socket_uids(child, depth + 1, nodes, receipt, outputs, inputs)?;
    }
    Ok(())
}

#[cfg(all(test, feature = "native-npc-economy"))]
#[path = "hunting_material_economy_tests.rs"]
pub(crate) mod tests;
