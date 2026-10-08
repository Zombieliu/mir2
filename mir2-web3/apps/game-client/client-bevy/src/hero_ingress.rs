//! Shared Hero raw ingress. No World, browser, transport, Native economy or catalogue lookup.
use crate::hero_model::{project_owner_hero, project_owner_item, HeroModel};
use crate::inventory::{CrystalUserItemModel, InventoryModel, ItemModel};
use mir2_protocol::{HeroUserInformation, UserItem};
use serde::{de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor}, Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use serde::de::Error as _;
use std::{collections::{BTreeSet, VecDeque}, fmt};

pub const MAX_RAW_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DEPTH: usize = 64;
pub const MAX_RECEIPTS: usize = 32;
pub const MAX_RETAINED_BYTES: usize = 32 * 1024 * 1024;
const MAX_MAP_BYTES: usize = 512;
const MAX_SAFE_JS: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeroScope {
    pub run_generation: u64,
    pub connection_generation: u64,
    pub session_generation: u64,
    pub scene_revision: u64,
    pub player_object_id: u32,
    pub map_file_name: String,
}
impl HeroScope {
    fn order(&self) -> [u64; 4] {
        [self.run_generation, self.connection_generation, self.session_generation, self.scene_revision]
    }
    pub fn valid(&self) -> bool {
        self.run_generation != 0 && self.connection_generation != 0 && self.session_generation != 0
            && self.order().iter().all(|n|*n<=MAX_SAFE_JS) && self.player_object_id != 0
            && !self.map_file_name.is_empty() && self.map_file_name.len() <= MAX_MAP_BYTES
            && !self.map_file_name.contains('\0')
    }
    pub fn same_physical(&self, other: &Self) -> bool {
        self.connection_generation==other.connection_generation && self.session_generation==other.session_generation
            && self.scene_revision==other.scene_revision && self.player_object_id==other.player_object_id
            && self.map_file_name==other.map_file_name
    }
    fn same_physical_session(&self, other: &Self) -> bool {
        self.run_generation == other.run_generation
            && self.connection_generation == other.connection_generation
            && self.session_generation == other.session_generation
            && self.player_object_id == other.player_object_id
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroIngressLease {
    pub generation: u64,
    pub scope: HeroScope,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroIngressError {
    Scope, Retired, Closed, Sequence, Clock, Exhausted, Json, Shape, Correlation,
    Projection, IncompleteInstance, DuplicatePlacement, Bounds, Undrained,
}

/// Bag, belt and quest only. It never claims to contain personal equipment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroPersonalBag {
    pub model: InventoryModel,
    pub equipment_excluded: bool,
    pub instance_metadata_complete: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroIngressDelivery {
    pub lease: HeroIngressLease,
    pub frame_sequence: u64,
    pub received_ms: u64,
    pub revision: u64,
    pub owner_current: bool,
    pub hero_instance_metadata_complete: bool,
    pub model: HeroModel,
    pub personal_bag: Option<HeroPersonalBag>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroIngressBatch {
    pub lease: HeroIngressLease,
    pub closed: bool,
    pub receipts: Vec<HeroIngressDelivery>,
    pub latest: Option<HeroIngressDelivery>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HeroIngressStatus {
    pub active: bool,
    pub closed: bool,
    pub owner_current: bool,
    pub frame_sequence: u64,
    pub receipt_count: usize,
    pub retained_bytes: usize,
}
#[derive(Clone)]
struct Candidate {
    model: HeroModel,
    owner: Option<HeroModel>,
    personal: Option<HeroPersonalBag>,
}
/// Explicit lifecycle owner. Incoming frames cannot activate/change this scope.
#[derive(Default)]
pub struct HeroIngress {
    lease: Option<HeroIngressLease>,
    high_scope: Option<[u64; 4]>,
    generation: u64,
    frame_sequence: u64,
    received_ms: u64,
    revision: u64,
    closed: bool,
    model: HeroModel,
    owner: Option<HeroModel>,
    personal: Option<HeroPersonalBag>,
    latest: Option<HeroIngressDelivery>,
    receipts: VecDeque<HeroIngressDelivery>,
    retained_bytes: usize,
}
impl HeroIngress {
    pub fn new() -> Self { Self::default() }

    /// Caller must derive this from its authenticated live connection/owner.
    /// Existing host generations must be ordered lexicographically as declared.
    pub fn activate(&mut self, scope: HeroScope) -> Result<(), HeroIngressError> {
        if !scope.valid() { return Err(HeroIngressError::Scope); }
        if let Some(current) = &self.lease {
            if current.scope == scope {
                return if self.closed { Err(HeroIngressError::Closed) } else { Ok(()) };
            }
        }
        if self.high_scope.is_some_and(|old| scope.order() <= old) {
            return Err(HeroIngressError::Retired);
        }
        let generation = next(self.generation)?;
        // Scene-only transition keeps a private packet bootstrap and original anchors.
        // It is inaccessible until a fresh matching owner arrives in the new scene.
        let carry = self.lease.as_ref().is_some_and(|old| old.scope.same_physical_session(&scope));
        let mut model = if carry { self.model.clone() } else { HeroModel::default() };
        model.snapshot_identity = None;
        model.stats = None;
        model.weights = None;
        model.learned_keys = None;
        model.skill_key_ack = None;
        model.item_result_receipt = false;
        model.last_item_result = None;
        model.inventory_view = InventoryModel::default();
        model.auto_pot_view = InventoryModel::default();
        model.actor_candidates.clear();
        model.session_epoch = generation;
        let lease = HeroIngressLease { generation, scope };
        let charge = measure(&model)?;
        if charge > MAX_RETAINED_BYTES { return Err(HeroIngressError::Bounds); }
        self.high_scope = Some(lease.scope.order());
        self.generation = generation;
        self.lease = Some(lease.clone());
        self.model = model;
        self.owner = None;
        self.personal = None;
        self.latest = None;
        self.receipts.clear();
        self.closed = false;
        self.retained_bytes = charge;
        // Global raw frame/time high waters survive every lifecycle boundary.
        Ok(())
    }
    pub fn scope(&self)->Option<&HeroScope> {self.lease.as_ref().map(|lease|&lease.scope)}
    pub fn is_current(&self,scope:&HeroScope)->bool {self.scope()==Some(scope)}
    pub fn batch_is_current(&self,batch:&HeroIngressBatch)->bool {
        if self.lease.as_ref()!=Some(&batch.lease) || batch.closed!=self.closed{return false;}
        let valid=|d:&HeroIngressDelivery|d.lease==batch.lease && d.frame_sequence>0 && d.frame_sequence<=self.frame_sequence
            && d.received_ms<=self.received_ms && d.revision<=self.revision;
        let mut previous=0;
        for delivery in &batch.receipts {if !valid(delivery)||delivery.frame_sequence<=previous{return false;}previous=delivery.frame_sequence;}
        batch.latest.as_ref().is_none_or(|latest|valid(latest)&&latest.frame_sequence>=previous)
    }
    pub fn frame_sequence(&self)->u64 {self.frame_sequence}
    pub fn model(&self)->HeroModel {
        Candidate{model:self.model.clone(),owner:self.owner.clone(),personal:self.personal.clone()}.public_model()
    }
    pub fn personal(&self)->Option<&InventoryModel> {self.personal.as_ref().map(|p|&p.model)}
    pub fn personal_source(&self)->Option<&HeroPersonalBag> {self.personal.as_ref()}
    pub fn available(&self)->bool {
        if self.lease.is_none()||self.closed||self.owner.is_none(){return false;}
        let model=self.model();
        model.snapshot_identity.is_none() || (packet_view_complete(&model)&&skills_complete(&model))
    }
    pub fn withdraw(&mut self, scope: &HeroScope) -> bool {
        if !self.is_current(scope) { return false; }
        self.lease = None;
        self.model = HeroModel::default();
        self.owner = None;
        self.personal = None;
        self.latest = None;
        self.receipts.clear();
        self.closed = true;
        self.retained_bytes = 0;
        true
    }
    pub fn status(&self) -> HeroIngressStatus {
        HeroIngressStatus { active:self.lease.is_some(), closed:self.closed,
            owner_current:!self.closed && self.owner.is_some(), frame_sequence:self.frame_sequence,
            receipt_count:self.receipts.len(), retained_bytes:self.retained_bytes }
    }
    pub fn receive_frame(&mut self, scope: &HeroScope, frame_sequence: u64,
        raw: &str, received_ms: u64) -> Result<bool, HeroIngressError> {
        if !self.is_current(scope) { return Err(HeroIngressError::Retired); }
        if self.closed { return Err(HeroIngressError::Closed); }
        if frame_sequence == 0 || frame_sequence > MAX_SAFE_JS || frame_sequence <= self.frame_sequence {
            return Err(HeroIngressError::Sequence);
        }
        if received_ms > MAX_SAFE_JS || received_ms < self.received_ms {
            return Err(HeroIngressError::Clock);
        }
        let lease=self.lease.as_ref().ok_or(HeroIngressError::Retired)?;
        let result = self.prepare(lease, frame_sequence, received_ms, raw);
        match result {
            Ok(None) => {
                self.frame_sequence = frame_sequence;
                self.received_ms = received_ms;
                Ok(false)
            }
            Ok(Some((candidate, mut delivery, publish))) => {
                let receipt = publish && (delivery.model.skill_key_ack.is_some() || delivery.model.item_result_receipt);
                if receipt && self.receipts.len() == MAX_RECEIPTS {
                    self.closed = true;
                    return Err(HeroIngressError::Bounds);
                }
                let mut latest = if publish { Some(delivery.clone()) } else { self.latest.clone() };
                // Receipt metadata is observed only on the explicit FIFO route.
                if let Some(latest) = latest.as_mut() {
                    latest.model.skill_key_ack = None;
                    latest.model.item_result_receipt = false;
                }
                let charge = self.candidate_charge(&candidate, latest.as_ref(), receipt.then_some(&delivery));
                let charge = match charge {
                    Ok(n) if n <= MAX_RETAINED_BYTES => n,
                    _ => { self.closed = true; return Err(HeroIngressError::Bounds); }
                };
                self.model = candidate.model;
                self.owner = candidate.owner;
                self.personal = candidate.personal;
                self.frame_sequence = frame_sequence;
                self.received_ms = received_ms;
                if publish {
                    self.revision = delivery.revision;
                    if receipt { self.receipts.push_back(delivery); }
                    self.latest = latest;
                } else {
                    // No candidate clock/serial was replayed merely for retention.
                    delivery.model.skill_key_ack = None;
                }
                self.retained_bytes = charge;
                Ok(publish)
            }
            Err(error) => {
                // Wrong map/actor cannot poison a newer current owner with an old snapshot.
                if error != HeroIngressError::Correlation { self.closed = true; }
                Err(error)
            }
        }
    }
    /// Even a closed lane may drain its accepted old FIFO; no new input is admitted.
    /// The actual World/host consumer MUST recheck is_current before application.
    pub fn take_batch(&mut self) -> Result<Option<HeroIngressBatch>, HeroIngressError> {
        let Some(lease)=self.lease.as_ref().cloned() else{return Ok(None);};
        if self.receipts.is_empty() && self.latest.is_none() { return Ok(None); }
        let retained=self.base_charge()?;
        let batch = HeroIngressBatch { lease:lease.clone(), closed:self.closed,
            receipts:self.receipts.drain(..).collect(), latest:self.latest.take() };
        self.retained_bytes = retained;
        Ok(Some(batch))
    }
    fn base_charge(&self) -> Result<usize, HeroIngressError> {
        add(measure(&self.model)?, add(measure(&self.owner)?, measure(&self.personal)?)?)
    }
    fn candidate_charge(&self, candidate:&Candidate, latest:Option<&HeroIngressDelivery>,
        receipt:Option<&HeroIngressDelivery>) -> Result<usize, HeroIngressError> {
        let mut n = add(measure(&candidate.model)?, add(measure(&candidate.owner)?, measure(&candidate.personal)?)?)?;
        for existing in &self.receipts { n = add(n, measure(existing)?)?; }
        if let Some(latest) = latest { n = add(n, measure(latest)?)?; }
        if let Some(receipt) = receipt { n = add(n, measure(receipt)?)?; }
        Ok(n)
    }
    fn prepare(&self, lease:&HeroIngressLease, frame_sequence:u64, received_ms:u64, raw:&str)
        -> Result<Option<(Candidate,HeroIngressDelivery,bool)>,HeroIngressError> {
        let value = strict_json(raw)?;
        let root = value.as_object().ok_or(HeroIngressError::Shape)?;
        let kind = text(root, "type")?;
        if kind != "worldSnapshot" && kind != "packet" { return Ok(None); }
        let payload = root.get("payload").and_then(Value::as_object).ok_or(HeroIngressError::Shape)?;
        let mut candidate = Candidate { model:self.model.clone(), owner:self.owner.clone(), personal:self.personal.clone() };
        let publish = if kind == "worldSnapshot" {
            if unsigned(payload,"playerObjectId")? != u64::from(lease.scope.player_object_id)
                || text(payload,"mapFileName")? != lease.scope.map_file_name {
                return Err(HeroIngressError::Correlation);
            }
            candidate.apply_owner(&Value::Object(payload.clone()))?;
            true
        } else {
            let packet = text(root,"packet")?;
            if !relevant_packet(packet) { return Ok(None); }
            validate_packet(packet, payload)?;
            room(&candidate.model)?;
            candidate.model.skill_key_ack = None;
            candidate.model.item_result_receipt = false;
            // Exactly one raw-arrival anchor. Drain never calls this again.
            let changed = candidate.model.apply_packet_at(packet, &Value::Object(payload.clone()), received_ms);
            candidate.reconcile_packet()?;
            changed
        };
        candidate.model.session_epoch = lease.generation;
        if publish { candidate.model.revision = next(self.model.revision)?; }
        let revision = if publish { next(self.revision)? } else { self.revision };
        let model = candidate.public_model();
        let delivery = HeroIngressDelivery { lease:lease.clone(), frame_sequence, received_ms, revision,
            owner_current:candidate.owner.is_some(), hero_instance_metadata_complete:packet_view_complete(&model),
            model, personal_bag:candidate.personal.clone() };
        Ok(Some((candidate,delivery,publish)))
    }
}

impl Candidate {
    fn apply_owner(&mut self, payload:&Value) -> Result<(),HeroIngressError> {
        room(&self.model)?;
        let mut source = project_owner_hero(payload).map_err(|_|HeroIngressError::Projection)?;
        validate_hero_placement(&source)?;
        let personal = project_personal_bag(payload)?;
        reject_cross_ids(&source.inventory_view, &personal.model)?;
        let old = self.model.clone();
        let identity_changed = self.owner.as_ref().and_then(|o|o.snapshot_identity.as_ref())
            .is_some_and(|previous| source.snapshot_identity.as_ref().is_none_or(|current|
                previous.name != current.name || previous.class != current.class || previous.gender != current.gender));
        let packet_matches = old.info.as_ref().zip(source.snapshot_identity.as_ref())
            .is_some_and(|(info,identity)| matches_identity(info,identity));
        source.hero_generation = if identity_changed { next(old.hero_generation)? }
            else { old.hero_generation };
        source.skill_snapshot_serial = next(old.skill_snapshot_serial)?;
        source.item_result_serial = old.item_result_serial;
        source.item_result_receipt = false;
        source.last_item_result = None;
        source.session_epoch = old.session_epoch;
        source.revision = old.revision;
        // UserItem values are reconstructed only from this actual owner's carriers.
        if packet_matches {
            let mut info = old.info.clone().ok_or(HeroIngressError::Projection)?;
            let identity = source.snapshot_identity.as_ref().ok_or(HeroIngressError::Projection)?;
            info.level = identity.level;
            info.experience = identity.experience;
            info.max_experience = identity.max_experience;
            if let Some(v) = identity.vitals { info.hp=v.hp; info.mp=v.mp; }
            info.auto_pot=identity.auto_pot; info.auto_hp_percent=identity.auto_hp_percent;
            info.auto_mp_percent=identity.auto_mp_percent;
            info.hp_item_index=identity.hp_item_index; info.mp_item_index=identity.mp_item_index;
            info.inventory = owner_user_array(&source.inventory_view,0,usize::from(source.inventory_view.capacity))?;
            info.equipment = owner_user_array(&source.inventory_view,2,14)?;
            if let Some(keys)=source.learned_keys.as_ref() {
                for magic in &mut info.magics {
                    if let Some(key)=keys.iter().find(|key|key.spell==magic.spell) { magic.key=key.key; }
                }
            }
            source.info=Some(info);
            source.base_stats=old.base_stats;
            source.magic_clocks=old.magic_clocks;
            source.hero_generation=source.hero_generation.max(old.hero_generation);
            source.riding_mount=old.riding_mount; source.fishing=old.fishing; source.poison=old.poison;
        } else if source.snapshot_identity.is_some() && old.info.is_some() {
            source.hero_generation=next(old.hero_generation)?;
        }
        if source.snapshot_identity.is_none() {
            source.skill_key_ack=None;
            source.actor_candidates.clear();
            source.magic_clocks.clear();
        }
        // No catalogue lookup/preview is performed in this module.
        source.auto_pot_view=InventoryModel::default();
        self.owner=Some(source.clone());
        self.model=source;
        self.personal=Some(personal);
        Ok(())
    }
    fn reconcile_packet(&mut self) -> Result<(),HeroIngressError> {
        let Some(source)=self.owner.as_ref() else { return Ok(()); };
        self.model.snapshot_identity=source.snapshot_identity.clone();
        self.model.stats=source.stats.clone(); self.model.weights=source.weights;
        self.model.learned_keys=source.learned_keys.clone();
        if self.model.info.as_ref().zip(source.snapshot_identity.as_ref())
            .is_some_and(|(info,identity)|matches_identity(info,identity)) {
            let info=self.model.info.as_ref().ok_or(HeroIngressError::Projection)?;
            self.model.inventory_view = packet_inventory_view(info,&source.inventory_view)?;
            if let Some(identity)=self.model.snapshot_identity.as_mut() {
                if let Some(vitals)=identity.vitals.as_mut() {vitals.hp=info.hp;vitals.mp=info.mp;}
            }
        }
        self.model.auto_pot_view=InventoryModel::default();
        Ok(())
    }
    fn public_model(&self) -> HeroModel {
        let mut model=self.model.clone();
        let matches = model.info.as_ref().zip(model.snapshot_identity.as_ref())
            .is_some_and(|(info,identity)|matches_identity(info,identity));
        if self.owner.is_none() || !matches {
            // Private scene/pre-owner bootstrap never constitutes current UI authority.
            model.info=None; model.base_stats=None; model.magic_clocks.clear();
            model.actor_candidates.clear(); model.skill_key_ack=None; model.item_result_receipt=false;
        }
        model
    }
}

fn matches_identity(info:&HeroUserInformation, source:&crate::hero_model::HeroSnapshotIdentity)->bool {
    info.name==source.name && info.class==source.class && info.gender==source.gender
}
fn next(n:u64)->Result<u64,HeroIngressError> {
    n.checked_add(1).filter(|n|*n!=u64::MAX).ok_or(HeroIngressError::Exhausted)
}
fn room(model:&HeroModel)->Result<(),HeroIngressError> {
    for n in [model.revision,model.hero_generation,model.skill_snapshot_serial,model.item_result_serial] {next(n)?;}
    Ok(())
}
fn add(left:usize,right:usize)->Result<usize,HeroIngressError> {
    left.checked_add(right).ok_or(HeroIngressError::Bounds)
}
fn text<'a>(object:&'a Map<String,Value>,key:&str)->Result<&'a str,HeroIngressError> {
    object.get(key).and_then(Value::as_str).ok_or(HeroIngressError::Shape)
}
fn unsigned(object:&Map<String,Value>,key:&str)->Result<u64,HeroIngressError> {
    object.get(key).and_then(Value::as_u64).ok_or(HeroIngressError::Shape)
}
fn signed(object:&Map<String,Value>,key:&str)->Result<i32,HeroIngressError> {
    object.get(key).and_then(Value::as_i64).and_then(|n|i32::try_from(n).ok()).ok_or(HeroIngressError::Shape)
}
fn boolean(object:&Map<String,Value>,key:&str)->Result<bool,HeroIngressError> {
    object.get(key).and_then(Value::as_bool).ok_or(HeroIngressError::Shape)
}
fn array<'a>(value:&'a Value,key:&str)->Result<&'a [Value],HeroIngressError> {
    value.get(key).and_then(Value::as_array).map(Vec::as_slice).ok_or(HeroIngressError::Shape)
}
fn relevant_packet(name:&str)->bool {
    matches!(name,"HeroInformation"|"HeroBaseStatsInfo"|"HeroHealthChanged"|"UpdateHeroSpawnState"
        |"ObjectHero"|"ObjectRemove"|"ObjectDied"|"MountUpdate"|"FishingUpdate"|"ObjectPoisoned"
        |"NewMagic"|"ObjectMagic"|"MagicDelay"|"MagicLeveled"|"MagicCast"
        |"MoveItem"|"EquipItem"|"RemoveItem"|"UseItem"|"MergeItem"|"TransferHeroItem"
        |"TakeBackHeroItem"|"DeleteItem"|"SetAutoPotValue"|"SetAutoPotItem")
}
fn validate_packet(name:&str,p:&Map<String,Value>)->Result<(),HeroIngressError> {
    match name {
        "MoveItem" if text(p,"grid")?=="HeroInventory" => {signed(p,"from")?;signed(p,"to")?;boolean(p,"success")?;}
        "EquipItem"|"RemoveItem" if text(p,"grid")?=="HeroInventory" => {unsigned(p,"uniqueId")?;signed(p,"to")?;boolean(p,"success")?;}
        "UseItem" if text(p,"grid")?=="HeroInventory" => {unsigned(p,"uniqueId")?;boolean(p,"success")?;}
        "MergeItem" => {text(p,"gridFrom")?;text(p,"gridTo")?;unsigned(p,"idFrom")?;unsigned(p,"idTo")?;boolean(p,"success")?;}
        "TransferHeroItem"|"TakeBackHeroItem" => {signed(p,"from")?;signed(p,"to")?;boolean(p,"success")?;}
        "DeleteItem" => {unsigned(p,"uniqueId")?;let count=unsigned(p,"count")?;u16::try_from(count).map_err(|_|HeroIngressError::Shape)?;}
        "SetAutoPotValue" => {let stat=unsigned(p,"stat")?;let value=unsigned(p,"value")?;if !matches!(stat,12|13)||value>99{return Err(HeroIngressError::Shape);}}
        "SetAutoPotItem" => {u8::try_from(unsigned(p,"grid")?).map_err(|_|HeroIngressError::Shape)?;signed(p,"item_index")?;}
        _ => {}
    }
    Ok(())
}

fn validate_hero_placement(model:&HeroModel)->Result<(),HeroIngressError> {
    if model.inventory_view.capacity>42 {return Err(HeroIngressError::Shape);}
    let mut cells=BTreeSet::new(); let mut ids=BTreeSet::new();
    for item in &model.inventory_view.items {
        let bound=match item.container {0=>u32::from(model.inventory_view.capacity),2=>14,_=>0};
        let uid=item.unique_id.ok_or(HeroIngressError::Shape)?;
        if item.slot>=bound || item.quantity==0 || !cells.insert((item.container,item.slot)) || (uid!=0&&!ids.insert(uid)) {
            return Err(HeroIngressError::DuplicatePlacement);
        }
    }
    let mut spells=BTreeSet::new();let mut keys=BTreeSet::new();
    for key in model.learned_keys.as_deref().unwrap_or_default() {
        let name=serde_json::to_string(&key.spell).map_err(|_|HeroIngressError::Projection)?;
        if !spells.insert(name) || (key.key!=0&&!keys.insert(key.key)) {return Err(HeroIngressError::DuplicatePlacement);}
    }
    if model.snapshot_identity.is_none() && (!model.inventory_view.items.is_empty()
        || !model.learned_keys.as_deref().unwrap_or_default().is_empty()) {
        return Err(HeroIngressError::Shape);
    }
    Ok(())
}
fn project_personal_bag(owner:&Value)->Result<HeroPersonalBag,HeroIngressError> {
    let object=owner.as_object().ok_or(HeroIngressError::Shape)?;
    let capacity=u16::try_from(unsigned(object,"inventoryCapacity")?).map_err(|_|HeroIngressError::Shape)?;
    let bag=unsigned(object,"maxBagSlots")?;
    if InventoryModel::canonical_capacity(capacity)!=capacity || u64::from(capacity).checked_sub(6)!=Some(bag) {
        return Err(HeroIngressError::Shape);
    }
    let gold=u32::try_from(unsigned(object,"gold")?).map_err(|_|HeroIngressError::Shape)?;
    // Presence of equipment is checked, but its differently shaped rows are NOT projected.
    array(owner,"equipmentItems")?;
    let mut model=InventoryModel {capacity,gold,items:Vec::new(),..Default::default()};
    let mut cells=BTreeSet::new();let mut ids=BTreeSet::new();
    for (field,belt) in [("inventoryItems",false),("beltItems",true)] {
        for raw in array(owner,field)? {
            let object=raw.as_object().ok_or(HeroIngressError::Shape)?;
            let raw_slot=unsigned(object,"slot")?;
            let (container,slot,bound)=if belt {
                if text(object,"container")?!="belt" {return Err(HeroIngressError::Shape);}
                (1,raw_slot,6)
            } else {
                match text(object,"container")? {
                    "bag1" if raw_slot<40=>(0,raw_slot,bag),
                    "bag2" if raw_slot<40=>(0,raw_slot.checked_add(40).ok_or(HeroIngressError::Shape)?,bag),
                    "quest"=>(3,raw_slot,40),
                    _=>return Err(HeroIngressError::Shape),
                }
            };
            let uid=unsigned(object,"uniqueId")?;
            if slot>=bound || !cells.insert((container,slot)) || (uid!=0&&!ids.insert(uid)) {
                return Err(HeroIngressError::DuplicatePlacement);
            }
            let item=project_owner_item(raw,container,slot).map_err(|_|HeroIngressError::Projection)?;
            if item.quantity==0 {return Err(HeroIngressError::Shape);}
            model.items.push(item);
        }
    }
    let complete=model.items.iter().all(instance_complete);
    Ok(HeroPersonalBag {model,equipment_excluded:true,instance_metadata_complete:complete})
}
fn instance_complete(item:&ItemModel)->bool {
    let Some(tooltip)=item.tooltip_source.as_ref() else {return false;};
    let Some(user)=tooltip.user_item.as_ref() else {return false;};
    item.unique_id==Some(user.unique_id) && item.quantity==u32::from(user.count) && user.count!=0
        && tooltip.info.item_index==user.item_index && socket_metadata_complete(tooltip)
}
fn complete_hero_items(model:&HeroModel)->bool {
    model.inventory_view.items.iter().all(instance_complete)
}
fn reject_cross_ids(hero:&InventoryModel,personal:&InventoryModel)->Result<(),HeroIngressError> {
    let mut ids=BTreeSet::new();
    for item in hero.items.iter().chain(personal.items.iter()) {
        let uid=item.unique_id.ok_or(HeroIngressError::Shape)?;
        if uid!=0&&!ids.insert(uid) {return Err(HeroIngressError::DuplicatePlacement);}
    }
    Ok(())
}
fn source_user(item:&ItemModel)->Result<Option<UserItem>,HeroIngressError> {
    if !instance_complete(item) {return Ok(None);}
    let source=item.tooltip_source.as_ref().and_then(|s|s.user_item.as_ref()).ok_or(HeroIngressError::IncompleteInstance)?;
    let raw=serde_json::to_value(source).map_err(|_|HeroIngressError::Projection)?;
    serde_json::from_value(raw).map(Some).map_err(|_|HeroIngressError::Projection)
}
/// Missing occupied instance metadata returns None, never a default/fake UserItem.
fn owner_user_array(model:&InventoryModel,container:u8,len:usize)->Result<Option<Vec<Option<UserItem>>>,HeroIngressError> {
    let mut result=vec![None;len];
    for item in model.items.iter().filter(|i|i.container==container) {
        let slot=usize::try_from(item.slot).map_err(|_|HeroIngressError::Shape)?;
        if slot>=len || result[slot].is_some() {return Err(HeroIngressError::DuplicatePlacement);}
        let Some(user)=source_user(item)? else {return Ok(None);};
        result[slot]=Some(user);
    }
    Ok(Some(result))
}
/// Only count/durability may reuse the same instance metadata between owner frames.
fn packet_metadata_matches(old:&CrystalUserItemModel,user:&UserItem)->bool {
    let (Ok(mut old),Ok(mut incoming))=(serde_json::to_value(old),serde_json::to_value(user)) else{return false;};
    for key in ["count","current_dura","max_dura"] {
        let (Some(a),Some(b))=(old.as_object_mut(),incoming.as_object_mut()) else{return false;};a.remove(key);b.remove(key);
    }
    old==incoming
}
fn packet_inventory_view(info:&HeroUserInformation,source:&InventoryModel)->Result<InventoryModel,HeroIngressError> {
    let mut model=InventoryModel {capacity:source.capacity,items:Vec::new(),..Default::default()};
    let mut ids=BTreeSet::new();
    for (container,section,bound) in [(0,info.inventory.as_ref(),usize::from(source.capacity)),(2,info.equipment.as_ref(),14)] {
        let Some(section)=section else {continue;};
        if section.len()!=bound {return Err(HeroIngressError::Shape);}
        for (slot,user) in section.iter().enumerate() {
            let Some(user)=user else {continue;};
            if user.count==0 || (user.unique_id!=0&&!ids.insert(user.unique_id)) {return Err(HeroIngressError::DuplicatePlacement);}
            // Same-owner instance ID AND item index identify the permitted metadata origin.
            let Some(known)=source.items.iter().find(|item|item.unique_id==Some(user.unique_id)
                && (user.unique_id!=0 || (item.container==container && item.slot==slot as u32))
                && item.tooltip_source.as_ref().and_then(|s|s.user_item.as_ref()).is_some_and(|old|old.item_index==user.item_index
                    && packet_metadata_matches(old,user)))
                else {continue;};
            if !instance_complete(known) {continue;}
            let mut item=known.clone();
            item.container=container;item.slot=u32::try_from(slot).map_err(|_|HeroIngressError::Shape)?;
            item.quantity=u32::from(user.count);item.durability_current=Some(user.current_dura);
            item.durability_max=Some(user.max_dura);
            let actual:CrystalUserItemModel=serde_json::from_value(serde_json::to_value(user).map_err(|_|HeroIngressError::Projection)?)
                .map_err(|_|HeroIngressError::Projection)?;
            let tooltip=item.tooltip_source.as_mut().ok_or(HeroIngressError::IncompleteInstance)?;
            tooltip.user_item=Some(actual);
            // Packet changes to socket layout cannot borrow old socket Info arrays.
            let old_user=known.tooltip_source.as_ref().and_then(|s|s.user_item.as_ref()).ok_or(HeroIngressError::IncompleteInstance)?;
            if old_user.slots!=tooltip.user_item.as_ref().ok_or(HeroIngressError::IncompleteInstance)?.slots {
                tooltip.socket_infos.clear();tooltip.real_socket_infos.clear();
            }
            item.icon=tooltip.user_item_image(item.quantity);
            model.items.push(item);
        }
    }
    Ok(model)
}
fn packet_view_complete(model:&HeroModel)->bool {
    if !complete_hero_items(model) {return false;}
    let Some(info)=model.info.as_ref() else {return false;};
    let Some(bag)=info.inventory.as_ref() else {return false;};
    let Some(equipment)=info.equipment.as_ref() else {return false;};
    let count=bag.iter().chain(equipment.iter()).filter(|x|x.is_some()).count();
    bag.len()==usize::from(model.inventory_view.capacity) && equipment.len()==14 && count==model.inventory_view.items.len()
}

// Conservatively account typed retained state as its exact serialized bytes plus
// a generous recursive Value allocation charge. Include live model/source/personal,
// all FIFO entries and latest; decoding/candidate temporaries are independently bounded.
fn value_charge(value:&Value)->Result<usize,HeroIngressError> {
    let mut n=128usize;
    match value {
        Value::String(s)=>n=add(n,s.len())?,
        Value::Array(values)=>for v in values {n=add(n,value_charge(v)?)?;},
        Value::Object(values)=>for (key,v) in values {n=add(n,add(key.len()+128,value_charge(v)?)?)?;},
        _=>{}
    }
    Ok(n)
}
fn measure<T:Serialize>(value:&T)->Result<usize,HeroIngressError> {
    let raw=serde_json::to_vec(value).map_err(|_|HeroIngressError::Projection)?;
    if raw.len()>MAX_RAW_BYTES {return Err(HeroIngressError::Bounds);}
    let tree=serde_json::to_value(value).map_err(|_|HeroIngressError::Projection)?;
    add(raw.len(),value_charge(&tree)?)
}

// Duplicate detection works on decoded keys, including escaped-key aliases.
struct StrictSeed<'a> {depth:usize,used:&'a mut usize}
struct StrictVisitor<'a> {depth:usize,used:&'a mut usize}
fn charge<E:de::Error>(used:&mut usize,n:usize)->Result<(),E> {
    *used=used.checked_add(n).ok_or_else(||E::custom("JSON allocation bound"))?;
    if *used>MAX_RETAINED_BYTES {return Err(E::custom("JSON allocation bound"));}
    Ok(())
}
impl<'de> DeserializeSeed<'de> for StrictSeed<'_> {
    type Value=Value;
    fn deserialize<D:de::Deserializer<'de>>(self,d:D)->Result<Value,D::Error> {
        if self.depth>MAX_DEPTH {return Err(D::Error::custom("JSON depth bound"));}
        charge::<D::Error>(self.used,128)?;
        d.deserialize_any(StrictVisitor{depth:self.depth,used:self.used})
    }
}
impl<'de> Visitor<'de> for StrictVisitor<'_> {
    type Value=Value;
    fn expecting(&self,f:&mut fmt::Formatter)->fmt::Result {f.write_str("bounded JSON without duplicate fields")}
    fn visit_bool<E:de::Error>(self,v:bool)->Result<Value,E> {Ok(Value::Bool(v))}
    fn visit_i64<E:de::Error>(self,v:i64)->Result<Value,E> {Ok(Value::Number(Number::from(v)))}
    fn visit_u64<E:de::Error>(self,v:u64)->Result<Value,E> {Ok(Value::Number(Number::from(v)))}
    fn visit_f64<E:de::Error>(self,v:f64)->Result<Value,E> {
        Number::from_f64(v).map(Value::Number).ok_or_else(||E::custom("invalid JSON number"))
    }
    fn visit_str<E:de::Error>(self,v:&str)->Result<Value,E> {charge::<E>(self.used,v.len())?;Ok(Value::String(v.into()))}
    fn visit_string<E:de::Error>(self,v:String)->Result<Value,E> {charge::<E>(self.used,v.len())?;Ok(Value::String(v))}
    fn visit_unit<E:de::Error>(self)->Result<Value,E> {Ok(Value::Null)}
    fn visit_none<E:de::Error>(self)->Result<Value,E> {Ok(Value::Null)}
    fn visit_seq<A:SeqAccess<'de>>(self,mut seq:A)->Result<Value,A::Error> {
        let mut values=Vec::new();
        while let Some(value)=seq.next_element_seed(StrictSeed{depth:self.depth+1,used:&mut *self.used})? {values.push(value);}
        Ok(Value::Array(values))
    }
    fn visit_map<A:MapAccess<'de>>(self,mut map:A)->Result<Value,A::Error> {
        let mut values=Map::new();
        while let Some(key)=map.next_key::<String>()? {
            charge::<A::Error>(self.used,key.len()+128)?;
            if values.contains_key(&key) {return Err(A::Error::custom("duplicate JSON field"));}
            let value=map.next_value_seed(StrictSeed{depth:self.depth+1,used:&mut *self.used})?;
            values.insert(key,value);
        }
        Ok(Value::Object(values))
    }
}
fn strict_json(raw:&str)->Result<Value,HeroIngressError> {
    if raw.is_empty()||raw.len()>MAX_RAW_BYTES {return Err(HeroIngressError::Bounds);}
    let mut parser=serde_json::Deserializer::from_str(raw);let mut used=0;
    let value=StrictSeed{depth:0,used:&mut used}.deserialize(&mut parser).map_err(|_|HeroIngressError::Json)?;
    parser.end().map_err(|_|HeroIngressError::Json)?;
    Ok(value)
}

// Not a network DTO. Root must keep the producing checkpoint string held in the
// same document and never route gateway data into restore_checkpoint.
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Checkpoint {
    version:u8,
    scope:HeroScope,
    frame_sequence:u64,
    received_ms:u64,
    revision:u64,
    closed:bool,
    model:HeroModel,
    owner:Option<HeroModel>,
    personal:Option<HeroPersonalBag>,
    latest:Option<HeroIngressDelivery>,
}
fn strip_receipt(model:&mut HeroModel) {
    model.skill_key_ack=None;model.item_result_receipt=false;model.last_item_result=None;
}
impl HeroIngress {
    pub fn checkpoint(&self)->Result<String,HeroIngressError> {
        if !self.receipts.is_empty(){return Err(HeroIngressError::Undrained);}
        let scope=self.scope().ok_or(HeroIngressError::Retired)?.clone();
        let mut model=self.model.clone();strip_receipt(&mut model);
        let mut owner=self.owner.clone();if let Some(owner)=owner.as_mut(){strip_receipt(owner);}
        let mut latest=self.latest.clone().or_else(||self.lease.as_ref().filter(|_|self.frame_sequence>0).map(|lease|HeroIngressDelivery{
            lease:lease.clone(),frame_sequence:self.frame_sequence,received_ms:self.received_ms,revision:self.revision,
            owner_current:self.owner.is_some(),hero_instance_metadata_complete:packet_view_complete(&self.model()),
            model:self.model(),personal_bag:self.personal.clone()}));
        if let Some(latest)=latest.as_mut(){strip_receipt(&mut latest.model);}
        let checkpoint=Checkpoint {version:1,scope,frame_sequence:self.frame_sequence,received_ms:self.received_ms,
            revision:self.revision,closed:self.closed,model,owner,personal:self.personal.clone(),latest};
        if measure(&checkpoint)?>MAX_RETAINED_BYTES {return Err(HeroIngressError::Bounds);}
        serde_json::to_string(&checkpoint).map_err(|_|HeroIngressError::Projection)
    }
    pub fn restore_checkpoint(&mut self,scope:&HeroScope,raw:&str)->Result<(),HeroIngressError> {
        if self.scope()!=Some(scope) {return Err(HeroIngressError::Retired);}
        if self.closed{return Err(HeroIngressError::Closed);}
        // Restore cannot replace state that has already consumed a live raw frame.
        if self.owner.is_some()||!self.receipts.is_empty()||self.latest.is_some() {return Err(HeroIngressError::Sequence);}
        let result=self.prepare_restore(scope,raw);
        match result {
            Ok((mut checkpoint,_old_charge))=> {
                let lease=self.lease.as_ref().ok_or(HeroIngressError::Retired)?.clone();
                checkpoint.model.session_epoch=lease.generation;
                if let Some(owner)=checkpoint.owner.as_mut(){owner.session_epoch=lease.generation;}
                if let Some(latest)=checkpoint.latest.as_mut(){
                    latest.lease=lease;latest.model.session_epoch=self.generation;
                }
                let candidate=Candidate{model:checkpoint.model.clone(),owner:checkpoint.owner.clone(),personal:checkpoint.personal.clone()};
                let charge=match self.candidate_charge(&candidate,checkpoint.latest.as_ref(),None) {
                    Ok(charge) if charge<=MAX_RETAINED_BYTES=>charge,
                    _=>{
                        self.model=HeroModel{session_epoch:self.generation,..Default::default()};
                        self.owner=None;self.personal=None;self.latest=None;self.receipts.clear();
                        self.closed=false;self.retained_bytes=measure(&self.model).unwrap_or(MAX_RETAINED_BYTES);
                        return Err(HeroIngressError::Bounds);
                    }
                };
                self.model=checkpoint.model;self.owner=checkpoint.owner;self.personal=checkpoint.personal;
                self.latest=checkpoint.latest;self.receipts.clear();
                self.frame_sequence=checkpoint.frame_sequence;self.received_ms=checkpoint.received_ms;
                self.revision=checkpoint.revision;self.closed=checkpoint.closed;self.retained_bytes=charge;
                Ok(())
            }
            Err(error)=>{
                self.model=HeroModel{session_epoch:self.generation,..Default::default()};
                self.owner=None;self.personal=None;self.latest=None;self.receipts.clear();
                self.closed=false;self.retained_bytes=measure(&self.model).unwrap_or(MAX_RETAINED_BYTES);
                Err(error)
            }
        }
    }
    fn prepare_restore(&self,scope:&HeroScope,raw:&str)->Result<(Checkpoint,usize),HeroIngressError> {
        let value=strict_json(raw)?;
        let checkpoint:Checkpoint=serde_json::from_value(value).map_err(|_|HeroIngressError::Shape)?;
        // Exact trusted Rust serialization prevents missing/default/unknown nested fields.
        if serde_json::to_string(&checkpoint).map_err(|_|HeroIngressError::Projection)?!=raw
            || checkpoint.version!=1 || !checkpoint.scope.valid()
            || !checkpoint.scope.same_physical(scope) || checkpoint.scope.run_generation>scope.run_generation {
            return Err(HeroIngressError::Correlation);
        }
        if checkpoint.frame_sequence<=self.frame_sequence || checkpoint.frame_sequence>MAX_SAFE_JS
            || checkpoint.received_ms<self.received_ms || checkpoint.received_ms>MAX_SAFE_JS {
            return Err(HeroIngressError::Sequence);
        }
        room(&checkpoint.model)?;next(checkpoint.revision)?;
        if checkpoint.model.magic_clocks.iter().any(|clock|clock.received_ms>checkpoint.received_ms){return Err(HeroIngressError::Clock);}
        if checkpoint.model.skill_key_ack.is_some()||checkpoint.model.item_result_receipt||checkpoint.model.last_item_result.is_some() {
            return Err(HeroIngressError::Shape);
        }
        if let Some(owner)=checkpoint.owner.as_ref(){
            validate_hero_placement(owner)?;
            if owner.skill_key_ack.is_some()||owner.item_result_receipt||owner.last_item_result.is_some(){return Err(HeroIngressError::Shape);}
        }
        if let Some(latest)=checkpoint.latest.as_ref(){
            if latest.lease.scope!=checkpoint.scope || latest.frame_sequence>checkpoint.frame_sequence
                || latest.received_ms>checkpoint.received_ms || latest.revision>checkpoint.revision
                || latest.model.skill_key_ack.is_some()||latest.model.item_result_receipt||latest.model.last_item_result.is_some(){
                return Err(HeroIngressError::Shape);
            }
        }
        let charge=measure(&checkpoint)?;
        if charge>MAX_RETAINED_BYTES {return Err(HeroIngressError::Bounds);}
        Ok((checkpoint,charge))
    }
}

fn socket_metadata_complete(source:&crate::inventory::CrystalItemTooltipSourceModel)->bool {
    let Some(user)=source.user_item.as_ref() else{return false;};
    if user.slots.is_empty(){return source.socket_infos.is_empty()&&source.real_socket_infos.is_empty();}
    source.socket_infos.len()==user.slots.len() && source.real_socket_infos.len()==user.slots.len()
        && user.slots.iter().zip(&source.socket_infos).zip(&source.real_socket_infos).all(|((slot,info),real)|
            match slot {None=>info.is_none()&&real.is_none(),Some(user)=>info.as_ref().is_some_and(|i|i.item_index==user.item_index)&&real.is_some()})
}
fn skills_complete(model:&HeroModel)->bool {
    let Some(info)=model.info.as_ref() else{return false;};
    let Some(keys)=model.learned_keys.as_ref() else{return false;};
    info.magics.len()==keys.len() && info.magics.iter().all(|magic|keys.iter().any(|key|key.spell==magic.spell&&key.key==magic.key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{CrystalItemInfoModel,CrystalItemTooltipSourceModel};
    use mir2_protocol::{ClientMagic,MirClass,MirGender};
    use serde_json::json;

    fn scope()->HeroScope { HeroScope {run_generation:1,connection_generation:1,session_generation:1,
        scene_revision:1,player_object_id:42,map_file_name:"TestMap".into()} }
    fn world()->Value {
        json!({"playerObjectId":42,"mapFileName":"TestMap","inventoryCapacity":46,"maxBagSlots":40,
            "gold":7,"inventoryItems":[],"beltItems":[],"equipmentItems":[],
            "heroMaxExperience":200,"heroVitals":{"hp":20,"maxHp":30,"mp":10,"maxMp":15},
            "heroStats":[],"heroWeights":{"bag":1,"wear":2,"hand":3},"heroInventoryCapacity":10,
            "heroInventoryItems":[],"heroEquipmentItems":[],"stage5Systems":{"heroLearnedMagics":[],
                "hero":{"name":"Hero","class":"Warrior","gender":"Male","level":2,"experience":77,
                "behaviour":0,"spawned":true,"autoPot":false,"autoHpPercent":30,"autoMpPercent":40,
                "hpItemIndex":0,"mpItemIndex":0}}})
    }
    fn item(uid:u64,slot:u8,container:&str)->Value {
        let info=CrystalItemInfoModel{item_index:1,name:"Actual fixture potion".into(),image:33,..Default::default()};
        let user=CrystalUserItemModel{unique_id:uid,item_index:1,count:2,..Default::default()};
        let tooltip=CrystalItemTooltipSourceModel{info:info.clone(),real_info:Some(info),user_item:Some(user),..Default::default()};
        json!({"uniqueId":uid,"key":uid.to_string(),"name":"Actual fixture potion","slot":slot,"container":container,
            "quantity":2,"icon":33,"description":"Full source fixture","durabilityCurrent":null,"durabilityMax":null,
            "sellValue":7,"grade":"common","addedAttack":0,"addedDefence":0,"tooltipSource":tooltip})
    }
    fn information()->HeroUserInformation {
        HeroUserInformation{object_id:12,name:"Hero".into(),class:MirClass::Warrior,gender:MirGender::Male,
            level:2,hair:3,hp:20,mp:10,experience:77,max_experience:200,inventory:Some(vec![None;10]),
            equipment:Some(vec![None;14]),magics:vec![],auto_pot:false,auto_hp_percent:30,auto_mp_percent:40,
            hp_item_index:0,mp_item_index:0}
    }
    fn magic()->ClientMagic {
        serde_json::from_value(json!({"name":"Fire Ball","spell":"FireBall","base_cost":1,"level_cost":0,"icon":1,
            "level1":1,"level2":2,"level3":3,"need1":1,"need2":2,"need3":3,"level":1,"key":17,
            "experience":0,"delay":3400,"range":8,"cast_time":-1000})).unwrap()
    }
    fn send_world(ingress:&mut HeroIngress,scope:&HeroScope,sequence:u64,owner:Value,at:u64)->Result<bool,HeroIngressError> {
        ingress.receive_frame(scope,sequence,&json!({"type":"worldSnapshot","payload":owner}).to_string(),at)
    }
    fn send_packet(ingress:&mut HeroIngress,scope:&HeroScope,sequence:u64,name:&str,payload:Value,at:u64)->Result<bool,HeroIngressError> {
        ingress.receive_frame(scope,sequence,&json!({"type":"packet","packet":name,"payload":payload}).to_string(),at)
    }
    fn start()->(HeroIngress,HeroScope) {
        let mut ingress=HeroIngress::default();let scope=scope();ingress.activate(scope.clone()).unwrap();
        assert!(send_world(&mut ingress,&scope,1,world(),100).unwrap());
        assert!(!ingress.available());
        assert!(send_packet(&mut ingress,&scope,2,"HeroInformation",json!({"info":information()}),100).unwrap());
        assert!(ingress.available());
        (ingress,scope)
    }
    #[test]
    fn hero_ingress_raw_world_and_information_preserve_exact_wide_values() {
        let mut ingress=HeroIngress::default();let scope=scope();ingress.activate(scope.clone()).unwrap();
        let mut owner=world();owner["stage5Systems"]["hero"]["experience"]=json!(i64::MIN);
        owner["heroMaxExperience"]=json!(i64::MAX);
        owner["heroInventoryItems"]=json!([item(u64::MAX,3,"bag1")]);
        owner["inventoryItems"]=json!([item(0,5,"bag1")]);
        assert!(send_world(&mut ingress,&scope,1,owner,10).unwrap());
        let mut info=information();info.experience=i64::MIN;info.max_experience=i64::MAX;
        let mut packet_user=source_user(&project_owner_item(&item(u64::MAX,3,"bag1"),0,3).unwrap()).unwrap().unwrap();
        packet_user.count=1;info.inventory.as_mut().unwrap()[3]=Some(packet_user);
        assert!(send_packet(&mut ingress,&scope,2,"HeroInformation",json!({"info":info}),20).unwrap());
        let model=ingress.model();let identity=model.snapshot_identity.unwrap();
        assert_eq!((identity.experience,identity.max_experience),(i64::MIN,i64::MAX));
        assert_eq!(model.info.as_ref().unwrap().inventory.as_ref().unwrap()[3].as_ref().unwrap().unique_id,u64::MAX);
        assert_eq!(model.inventory_view.items[0].unique_id,Some(u64::MAX));
        assert_eq!(model.inventory_view.items[0].quantity,1);
        assert_eq!(ingress.personal().unwrap().items[0].unique_id,Some(0));
        assert!(ingress.personal_source().unwrap().equipment_excluded);
        assert!(ingress.available());
        // Later actual owner wins count/dynamic instance state without inventing any UserItem.
        let mut owner=world();owner["heroInventoryItems"]=json!([item(u64::MAX,3,"bag1")]);
        owner["inventoryItems"]=json!([item(0,5,"bag1")]);
        send_world(&mut ingress,&scope,3,owner,30).unwrap();
        assert_eq!(ingress.model().info.unwrap().inventory.unwrap()[3].as_ref().unwrap().count,2);
        let mut no_hero=world();no_hero["stage5Systems"]["hero"]=Value::Null;no_hero["heroMaxExperience"]=Value::Null;
        no_hero["inventoryItems"]=json!([item(0,5,"bag1")]);
        send_world(&mut ingress,&scope,4,no_hero,40).unwrap();
        assert!(ingress.model().info.is_none()&&ingress.model().magic_clocks.is_empty());
        assert_eq!(ingress.personal().unwrap().items[0].unique_id,Some(0));
    }
    #[test]
    fn hero_ingress_strict_raw_parser_rejects_duplicates_depth_bounds_and_trailing() {
        let duplicate=r#"{"type":"worldSnapshot","ty\u0070e":"packet","payload":{}}"#;
        assert_eq!(strict_json(duplicate).unwrap_err(),HeroIngressError::Json);
        assert_eq!(strict_json("{} {}").unwrap_err(),HeroIngressError::Json);
        assert_eq!(strict_json(&("[".repeat(MAX_DEPTH+1)+"0"+&"]".repeat(MAX_DEPTH+1))).unwrap_err(),HeroIngressError::Json);
        assert_eq!(strict_json(&" ".repeat(MAX_RAW_BYTES+1)).unwrap_err(),HeroIngressError::Bounds);
        let (mut ingress,scope)=start();let sequence=ingress.frame_sequence();
        let mut wrong=world();wrong["mapFileName"]=json!("OldMap");
        assert_eq!(send_world(&mut ingress,&scope,3,wrong,101),Err(HeroIngressError::Correlation));
        assert_eq!(ingress.frame_sequence(),sequence);assert!(ingress.available());
        let mut wrong=world();wrong["playerObjectId"]=json!(43);
        assert_eq!(send_world(&mut ingress,&scope,3,wrong,101),Err(HeroIngressError::Correlation));
        assert!(ingress.available());
        let mut wrong=world();wrong.as_object_mut().unwrap().remove("heroMaxExperience");
        assert_eq!(send_world(&mut ingress,&scope,3,wrong,101),Err(HeroIngressError::Projection));
        assert!(ingress.status().closed);assert_eq!(ingress.frame_sequence(),sequence);
        assert!(!ingress.available());
    }
    #[test]
    fn hero_ingress_exact_skill_and_item_receipts_are_fifo_before_latest() {
        let (mut ingress,scope)=start();ingress.take_batch().unwrap();
        let mut info=information();info.magics.push(magic());
        send_packet(&mut ingress,&scope,3,"HeroInformation",json!({"info":info}),101).unwrap();
        let mut owner=world();owner["stage5Systems"]["heroLearnedMagics"]=json!([{"spell":"FireBall","key":17}]);
        send_world(&mut ingress,&scope,4,owner.clone(),102).unwrap();
        let baseline=ingress.model();ingress.take_batch().unwrap();
        send_packet(&mut ingress,&scope,5,"UseItem",json!({"uniqueId":u64::MAX,"grid":"HeroInventory","success":false}),103).unwrap();
        send_packet(&mut ingress,&scope,6,"SetAutoPotValue",json!({"stat":12,"value":35,"typed":true}),104).unwrap();
        owner["stage5Systems"]["heroLearnedMagics"]=json!([{"spell":"FireBall","key":18}]);
        owner["skillKeyAck"]=json!({"requestId":51,"spell":"FireBall","key":18,"oldKey":17,"accepted":true});
        send_world(&mut ingress,&scope,7,owner,105).unwrap();
        send_packet(&mut ingress,&scope,8,"HeroHealthChanged",json!({"hp":9,"mp":4}),106).unwrap();
        let batch=ingress.take_batch().unwrap().unwrap();
        assert!(ingress.batch_is_current(&batch));assert_eq!(batch.receipts.len(),3);
        assert_eq!(batch.receipts.iter().map(|r|r.frame_sequence).collect::<Vec<_>>(),vec![5,6,7]);
        assert_eq!(batch.receipts[0].model.last_item_result.as_ref().unwrap().1["uniqueId"].as_u64(),Some(u64::MAX));
        assert_eq!(batch.receipts[1].model.last_item_result.as_ref().unwrap().0,"SetAutoPotValue");
        let pending=crate::hero_model::HeroKeyPending {hero_generation:baseline.hero_generation,request_id:51,
            session_epoch:baseline.session_epoch,object_id:12,snapshot_serial:baseline.skill_snapshot_serial,
            spell:mir2_protocol::Spell::FireBall,key:18,old_key:17};
        assert_eq!(pending.outcome(&batch.receipts[2].model),Some(crate::hero_model::HeroKeyOutcome::Applied));
        let mut wrong=pending.clone();wrong.request_id=52;
        assert_eq!(wrong.outcome(&batch.receipts[2].model),None);
        let latest=batch.latest.unwrap();assert_eq!(latest.frame_sequence,8);
        assert!(!latest.model.item_result_receipt&&latest.model.skill_key_ack.is_none());
        assert_eq!(latest.model.info.unwrap().hp,9);assert!(ingress.take_batch().unwrap().is_none());
    }
    #[test]
    fn hero_ingress_clock_anchor_survives_owner_updates_drain_and_scene_carry() {
        let (mut ingress,scope)=start();let mut info=information();info.magics.push(magic());
        send_packet(&mut ingress,&scope,3,"HeroInformation",json!({"info":info}),100).unwrap();
        let mut owner=world();owner["stage5Systems"]["heroLearnedMagics"]=json!([{"spell":"FireBall","key":17}]);
        send_world(&mut ingress,&scope,4,owner.clone(),200).unwrap();
        assert_eq!(ingress.model().magic_clocks[0].received_ms,100);
        assert_eq!(ingress.model().magic_clocks[0].remaining_ms(1100),1400);
        ingress.take_batch().unwrap();assert_eq!(ingress.model().magic_clocks[0].received_ms,100);
        assert!(!send_packet(&mut ingress,&scope,5,"MagicCast",json!({"spell":"FireBall"}),1100).unwrap());
        assert!(!send_packet(&mut ingress,&scope,6,"ObjectMagic",json!({"objectId":11,"spell":"FireBall","cast":true}),1101).unwrap());
        assert_eq!(ingress.model().magic_clocks[0].received_ms,100);
        send_packet(&mut ingress,&scope,7,"ObjectMagic",json!({"objectId":12,"spell":"FireBall","cast":true}),1200).unwrap();
        send_packet(&mut ingress,&scope,8,"MagicDelay",json!({"objectId":12,"spell":"FireBall","delay":2000}),1300).unwrap();
        assert_eq!(ingress.model().magic_clocks[0].remaining_ms(1300),1900);
        let mut next=scope.clone();next.scene_revision+=1;next.map_file_name="Destination".into();
        ingress.activate(next.clone()).unwrap();assert!(!ingress.available());assert!(ingress.model().info.is_none());
        owner["mapFileName"]=json!("Destination");
        send_world(&mut ingress,&next,9,owner,1400).unwrap();
        assert!(ingress.available());assert_eq!(ingress.model().magic_clocks[0].received_ms,1200);
        assert_eq!(ingress.model().magic_clocks[0].remaining_ms(1400),1800);
    }
    #[test]
    fn hero_ingress_scope_sequence_clock_and_retirement_high_waters_are_exact() {
        let (mut ingress,scope)=start();let batch=ingress.take_batch().unwrap().unwrap();
        assert_eq!(send_packet(&mut ingress,&scope,2,"MagicCast",json!({}),101),Err(HeroIngressError::Sequence));
        assert_eq!(send_packet(&mut ingress,&scope,3,"MagicCast",json!({}),99),Err(HeroIngressError::Clock));
        assert!(ingress.withdraw(&scope));assert!(!ingress.batch_is_current(&batch));
        assert_eq!(ingress.activate(scope.clone()),Err(HeroIngressError::Retired));
        assert_eq!(send_world(&mut ingress,&scope,3,world(),101),Err(HeroIngressError::Retired));
        let mut next=scope.clone();next.connection_generation=2;ingress.activate(next.clone()).unwrap();
        assert!(ingress.model().info.is_none());assert_eq!(ingress.frame_sequence(),2);
        assert_eq!(send_world(&mut ingress,&next,2,world(),101),Err(HeroIngressError::Sequence));
        send_world(&mut ingress,&next,3,world(),101).unwrap();assert!(!ingress.available());
        let mut invalid=next.clone();invalid.run_generation=MAX_SAFE_JS+1;
        assert_eq!(ingress.activate(invalid),Err(HeroIngressError::Scope));
        ingress.model.revision=u64::MAX-1;
        assert_eq!(send_world(&mut ingress,&next,4,world(),102),Err(HeroIngressError::Exhausted));
        assert!(ingress.status().closed);assert_eq!(ingress.frame_sequence(),3);
    }
    #[test]
    fn hero_ingress_personal_bag_is_strict_and_explicitly_excludes_equipment() {
        let mut owner=world();owner["inventoryCapacity"]=json!(54);owner["maxBagSlots"]=json!(48);
        owner["inventoryItems"]=json!([item(u64::MAX,0,"bag2"),item(0,2,"quest")]);
        owner["beltItems"]=json!([item(2,5,"belt")]);
        owner["equipmentItems"]=json!([{"slot":"weapon","name":"Excluded equipment"}]);
        let projection=project_personal_bag(&owner).unwrap();
        assert!(projection.equipment_excluded&&projection.instance_metadata_complete);
        assert_eq!(projection.model.items.iter().map(|i|(i.container,i.slot,i.unique_id)).collect::<Vec<_>>(),
            vec![(0,40,Some(u64::MAX)),(3,2,Some(0)),(1,5,Some(2))]);
        let mut duplicate=owner.clone();duplicate["inventoryItems"]=json!([item(1,0,"bag1"),item(3,0,"bag1")]);
        assert!(matches!(project_personal_bag(&duplicate),Err(HeroIngressError::DuplicatePlacement)));
        let mut conflict=owner.clone();conflict["beltItems"]=json!([item(u64::MAX,5,"belt")]);
        assert!(matches!(project_personal_bag(&conflict),Err(HeroIngressError::DuplicatePlacement)));
        let mut incomplete=owner;incomplete["inventoryItems"][0].as_object_mut().unwrap().remove("tooltipSource");
        let partial=project_personal_bag(&incomplete).unwrap();assert!(!partial.instance_metadata_complete);
        // No auto-pot preview or personal equipment is silently injected into these grids.
        assert!(partial.model.items.iter().all(|i|i.container!=2));
        let mut wrong=world();wrong["inventoryCapacity"]=json!(47);
        assert!(matches!(project_personal_bag(&wrong),Err(HeroIngressError::Shape)));
    }
    #[test]
    fn hero_ingress_receipt_and_byte_overflow_preserve_accepted_fifo_and_close() {
        let (mut ingress,scope)=start();ingress.take_batch().unwrap();
        for n in 0..MAX_RECEIPTS {send_packet(&mut ingress,&scope,3+n as u64,"SetAutoPotValue",
            json!({"stat":12,"value":n as u32}),101+n as u64).unwrap();}
        let before=ingress.frame_sequence();assert_eq!(ingress.status().receipt_count,MAX_RECEIPTS);
        assert_eq!(send_packet(&mut ingress,&scope,before+1,"SetAutoPotValue",json!({"stat":12,"value":50}),200),Err(HeroIngressError::Bounds));
        assert_eq!(ingress.frame_sequence(),before);assert!(ingress.status().closed);assert!(!ingress.available());
        let batch=ingress.take_batch().unwrap().unwrap();assert!(batch.closed);assert_eq!(batch.receipts.len(),MAX_RECEIPTS);
        assert_eq!(batch.receipts[0].model.last_item_result.as_ref().unwrap().1["value"],json!(0));
        assert_eq!(batch.receipts.last().unwrap().model.last_item_result.as_ref().unwrap().1["value"],json!(31));
        assert!(ingress.take_batch().unwrap().is_none());
        let (mut byte_limited,scope)=start();let before=byte_limited.frame_sequence();
        let mut large=world();large["heroInventoryItems"]=json!([item(1,0,"bag1")]);
        large["heroInventoryItems"][0]["description"]=json!("X".repeat(6*1024*1024));
        assert!(json!({"type":"worldSnapshot","payload":large.clone()}).to_string().len()<=MAX_RAW_BYTES);
        // Multiple live/source/delivery copies exceed retained accounting while raw remains <=16MiB.
        assert_eq!(send_world(&mut byte_limited,&scope,3,large,200),Err(HeroIngressError::Bounds));
        assert_eq!(byte_limited.frame_sequence(),before);assert!(byte_limited.status().closed);
        assert!(byte_limited.take_batch().unwrap().is_some());
    }
    #[test]
    fn hero_ingress_checkpoint_handoff_preserves_first_clock_without_receipt_replay() {
        let (mut old,scope)=start();let mut info=information();info.magics.push(magic());
        send_packet(&mut old,&scope,3,"HeroInformation",json!({"info":info}),100).unwrap();
        let mut owner=world();owner["stage5Systems"]["heroLearnedMagics"]=json!([{"spell":"FireBall","key":17}]);
        send_world(&mut old,&scope,4,owner,200).unwrap();
        send_packet(&mut old,&scope,5,"SetAutoPotValue",json!({"stat":12,"value":35}),300).unwrap();
        assert_eq!(old.status().receipt_count,1);
        assert_eq!(old.checkpoint(),Err(HeroIngressError::Undrained));
        let consumed=old.take_batch().unwrap().unwrap();assert_eq!(consumed.receipts.len(),1);assert!(old.batch_is_current(&consumed));
        let raw=old.checkpoint().unwrap();let mut next=scope.clone();next.run_generation=2;
        let mut new=HeroIngress::default();new.activate(next.clone()).unwrap();new.restore_checkpoint(&next,&raw).unwrap();
        assert!(new.available());assert_eq!(new.frame_sequence(),5);assert_eq!(new.status().receipt_count,0);
        assert_eq!(new.model().magic_clocks[0].received_ms,100);
        assert_eq!(new.model().magic_clocks[0].remaining_ms(1100),1400);
        assert!(new.model().last_item_result.is_none());
        let batch=new.take_batch().unwrap().unwrap();assert!(batch.receipts.is_empty());
        assert_eq!(batch.latest.unwrap().lease.scope,next);assert!(new.take_batch().unwrap().is_none());
        assert_eq!(send_packet(&mut new,&next,5,"HeroInformation",json!({"info":information()}),1100),Err(HeroIngressError::Sequence));
        let mut other=scope.clone();other.run_generation=2;other.scene_revision=2;
        let mut wrong=HeroIngress::default();wrong.activate(other.clone()).unwrap();
        assert_eq!(wrong.restore_checkpoint(&other,&raw),Err(HeroIngressError::Correlation));assert!(!wrong.available());
        send_world(&mut wrong,&other,6,world(),400).unwrap();assert!(!wrong.available());
        send_packet(&mut wrong,&other,7,"HeroInformation",json!({"info":information()}),401).unwrap();assert!(wrong.available());
        let mut malformed=HeroIngress::default();malformed.activate(next.clone()).unwrap();
        assert_eq!(malformed.restore_checkpoint(&next,&(raw+" ")),Err(HeroIngressError::Correlation));assert!(!malformed.available());
    }
    #[test]
    fn hero_ingress_closed_phase_rejects_older_batch_and_restore() {
        let (mut ingress,scope)=start();let batch=ingress.take_batch().unwrap().unwrap();let checkpoint=ingress.checkpoint().unwrap();
        assert!(ingress.batch_is_current(&batch));
        assert_eq!(ingress.receive_frame(&scope,3,"{",101),Err(HeroIngressError::Json));
        assert!(!ingress.batch_is_current(&batch));assert!(!ingress.available());
        assert_eq!(ingress.restore_checkpoint(&scope,&checkpoint),Err(HeroIngressError::Closed));
        assert!(ingress.status().closed);
    }
    #[test]
    fn hero_ingress_anonymous_items_are_slot_bound_and_dynamic_metadata_waits_for_owner() {
        let mut ingress=HeroIngress::default();let scope=scope();ingress.activate(scope.clone()).unwrap();let mut owner=world();
        owner["heroInventoryItems"]=json!([item(0,3,"bag1"),item(0,4,"bag1")]);
        owner["inventoryItems"]=json!([item(0,5,"bag1"),item(0,6,"bag1")]);send_world(&mut ingress,&scope,1,owner,10).unwrap();
        let mut info=information();let user=source_user(&project_owner_item(&item(0,3,"bag1"),0,3).unwrap()).unwrap().unwrap();
        info.inventory.as_mut().unwrap()[3]=Some(user.clone());info.inventory.as_mut().unwrap()[4]=Some(user.clone());
        send_packet(&mut ingress,&scope,2,"HeroInformation",json!({"info":info.clone()}),20).unwrap();assert!(ingress.available());
        assert_eq!(ingress.model().inventory_view.items.len(),2);assert_eq!(ingress.personal().unwrap().items.len(),2);
        info.inventory.as_mut().unwrap()[3]=None;info.inventory.as_mut().unwrap()[2]=Some(user);
        send_packet(&mut ingress,&scope,3,"HeroInformation",json!({"info":info}),30).unwrap();assert!(!ingress.available());
        let (mut changed,scope)=start();let mut owner=world();owner["heroInventoryItems"]=json!([item(17,3,"bag1")]);
        send_world(&mut changed,&scope,3,owner,101).unwrap();let mut info=information();
        let mut user=source_user(&project_owner_item(&item(17,3,"bag1"),0,3).unwrap()).unwrap().unwrap();user.soul_bound_id=999;
        info.inventory.as_mut().unwrap()[3]=Some(user);send_packet(&mut changed,&scope,4,"HeroInformation",json!({"info":info}),102).unwrap();
        assert!(!changed.available());assert!(changed.model().inventory_view.items.is_empty());
        let (mut lengths,scope)=start();let mut info=information();info.inventory.as_mut().unwrap().push(None);
        assert_eq!(send_packet(&mut lengths,&scope,3,"HeroInformation",json!({"info":info}),101),Err(HeroIngressError::Shape));assert!(lengths.status().closed);
    }

}
