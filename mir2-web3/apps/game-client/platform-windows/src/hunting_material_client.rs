//! Pure native material custody. This file performs no network send or gameplay
//! mutation. The future physical owner must publish staged inventory only after
//! its complete producer actually enters the gameplay queue, and call `enter`
//! inside the final owner fence directly before `start_send` without an await.
//!
//! Committed is an immutable receipt, never Applied. Completion accepts the
//! independent runtime's private MaterialApplied proof after its real complete
//! World commit and reauthorization under the physical owner's current gate.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use crate::gateway::{NativeCommandFence, NativeCommandStamp};
use mir2_client_bevy::inventory::InventoryModel;
use mir2_client_wire::{hunting_material as wire, U64};
use serde_json::Value;

type Result<T> = std::result::Result<T, &'static str>;
const CONTROL_TIMEOUT_MS: u64 = 10_000;
const MAX_ACTORS: usize = 32;
const MAX_CONFIRMED_QUOTES: usize = 4096;
const MAX_INVENTORY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MaterialActor {
    realm: String,
    account_id: String,
    character_index: i32,
}
impl MaterialActor {
    pub(crate) fn new(realm: String, account_id: String, character_index: i32) -> Result<Self> {
        if !valid_text(&realm, 128) || !valid_text(&account_id, 160) || character_index < 0 {
            return Err("Invalid material actor");
        }
        Ok(Self {
            realm,
            account_id,
            character_index,
        })
    }
    pub(crate) fn realm(&self) -> &str {
        &self.realm
    }
    pub(crate) fn account_id(&self) -> &str {
        &self.account_id
    }
    pub(crate) fn character_index(&self) -> i32 {
        self.character_index
    }
}
fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

/// Physical-reader fact, not a server-side authority or bearer credential. It
/// has no deserializer, boolean enable flag or relationship to npcRepairMode.
#[derive(Clone)]
pub(crate) struct MaterialConnection {
    owner: NativeCommandStamp,
    actor: MaterialActor,
    namespace: Arc<()>,
}
impl std::fmt::Debug for MaterialConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialConnection")
            .field("owner", &self.owner)
            .field("actor", &self.actor)
            .finish_non_exhaustive()
    }
}
impl MaterialConnection {
    /// Only the actual reader/gateway may call this after its strict
    /// realmInfo.huntingMaterialOwnerProtocolVersion=1, verified identitySession
    /// and complete entry agree with this account/actor. The final send must
    /// still recheck the same real fence. No fence is retained or recursively
    /// locked by pure custody while that final physical lock is held.
    pub(super) fn from_verified_reader(
        owner: NativeCommandStamp,
        actor: MaterialActor,
        advertised_protocol_version: u16,
        fence: &NativeCommandFence,
    ) -> Result<Self> {
        if advertised_protocol_version != wire::PROTOCOL_VERSION || !fence.accepts(owner, true) {
            return Err("Material protocol or physical owner unavailable");
        }
        Ok(Self {
            owner,
            actor,
            namespace: Arc::new(()),
        })
    }
    #[cfg(test)]
    fn for_test(owner: NativeCommandStamp, actor: MaterialActor, version: u16) -> Result<Self> {
        if version != wire::PROTOCOL_VERSION {
            return Err("Unsupported material protocol");
        }
        Ok(Self {
            owner,
            actor,
            namespace: Arc::new(()),
        })
    }
    pub(crate) fn owner(&self) -> NativeCommandStamp {
        self.owner
    }
    pub(crate) fn actor(&self) -> &MaterialActor {
        &self.actor
    }
    fn same(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.actor == other.actor
            && Arc::ptr_eq(&self.namespace, &other.namespace)
    }
}

#[derive(Debug, Default)]
struct SourceState {
    connection: Option<MaterialConnection>,
    revision: u64,
    publication: u64,
    exhausted: bool,
    inventory: Option<Arc<Value>>,
    ready: bool,
}
impl SourceState {
    fn advance_revision(&mut self) -> Result<()> {
        match self.revision.checked_add(1) {
            Some(next) if !self.exhausted => {
                self.revision = next;
                Ok(())
            }
            _ => {
                self.exhausted = true;
                self.ready = false;
                Err("Material source revision exhausted")
            }
        }
    }
    fn next_publication(&mut self) -> Result<()> {
        match self.publication.checked_add(1) {
            Some(next) if !self.exhausted => {
                self.publication = next;
                Ok(())
            }
            _ => {
                self.exhausted = true;
                self.ready = false;
                Err("Material source publication exhausted")
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MaterialSourceTracker {
    state: Arc<Mutex<SourceState>>,
}
/// A staged complete inventory, not delivery or source eligibility. Consume
/// only after the exact full producer was successfully queued. No Deserialize.
pub(crate) struct MaterialInventoryCandidate {
    tracker: Arc<Mutex<SourceState>>,
    connection: MaterialConnection,
    revision: u64,
    publication: u64,
    inventory: Arc<Value>,
}
impl std::fmt::Debug for MaterialInventoryCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MaterialInventoryCandidate(<private>)")
    }
}
/// Opaque local source preimage; fields cannot be populated by packet JSON.
#[derive(Clone)]
pub(crate) struct MaterialSource {
    tracker: Arc<Mutex<SourceState>>,
    connection: MaterialConnection,
    revision: u64,
    inventory: Arc<Value>,
    unique_id: u64,
    input_count: u64,
    quantity: u64,
}
impl std::fmt::Debug for MaterialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialSource")
            .field("owner", &self.connection.owner)
            .field("unique_id", &self.unique_id)
            .field("input_count", &self.input_count)
            .field("quantity", &self.quantity)
            .finish_non_exhaustive()
    }
}
impl MaterialSource {
    pub(crate) fn owner(&self) -> NativeCommandStamp {
        self.connection.owner
    }
    pub(crate) fn unique_id(&self) -> u64 {
        self.unique_id
    }
    pub(crate) fn input_count(&self) -> u64 {
        self.input_count
    }
    pub(crate) fn quantity(&self) -> u64 {
        self.quantity
    }
    pub(super) fn current(&self) -> bool {
        self.tracker.lock().is_ok_and(|state| {
            !state.exhausted
                && state.ready
                && state.revision == self.revision
                && state
                    .connection
                    .as_ref()
                    .is_some_and(|c| c.same(&self.connection))
                && state
                    .inventory
                    .as_ref()
                    .is_some_and(|image| image == &self.inventory)
        })
    }
    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tracker, &other.tracker)
            && self.connection.same(&other.connection)
            && self.revision == other.revision
            && self.inventory == other.inventory
            && self.unique_id == other.unique_id
            && self.input_count == other.input_count
            && self.quantity == other.quantity
    }
}

fn inventory_image(inventory: &InventoryModel) -> Result<Value> {
    if inventory.capacity != InventoryModel::canonical_capacity(inventory.capacity)
        || inventory.items.len() > 256
    {
        return Err("Invalid complete material inventory");
    }
    let mut slots = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for item in &inventory.items {
        let valid_slot = match item.container {
            0 => item.slot < u32::from(inventory.bag_slot_capacity()),
            1 => item.slot < 6,
            2 => item.slot < 14,
            3 => item.slot < 80,
            _ => false,
        };
        if !valid_slot
            || item.quantity == 0
            || !slots.insert((item.container, item.slot))
            || item.unique_id.is_some_and(|uid| !identities.insert(uid))
            || (item.container != 2 && item.unique_id.is_none())
        {
            return Err("Ambiguous material inventory custody");
        }
    }
    let image =
        serde_json::to_value(inventory).map_err(|_| "Material inventory encoding failed")?;
    if serde_json::to_vec(&image)
        .map_err(|_| "Material inventory encoding failed")?
        .len()
        > MAX_INVENTORY_BYTES
    {
        return Err("Material inventory exceeds custody limit");
    }
    Ok(image)
}

impl MaterialSourceTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    /// The caller first verifies the complete raw owner inventory (including
    /// rich metadata and duplicate keys). A tolerant renderer mapping alone
    /// cannot establish completeness. Ordinary equal snapshots do not advance
    /// ABA revision; actual mutation events must call invalidate even when their
    /// visible value is unchanged or is subsequently restored.
    pub(crate) fn stage_inventory(
        &self,
        connection: &MaterialConnection,
        inventory: &InventoryModel,
    ) -> Result<MaterialInventoryCandidate> {
        // Invalid new data revokes an older ready source before returning.
        let image = inventory_image(inventory);
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Material source unavailable")?;
        state.ready = false;
        state.next_publication()?;
        let image = match image {
            Ok(image) => image,
            Err(error) => {
                let _ = state.advance_revision();
                return Err(error);
            }
        };
        if state
            .connection
            .as_ref()
            .is_none_or(|prior| !prior.same(connection))
            || state.inventory.as_deref() != Some(&image)
        {
            state.advance_revision()?;
        }
        let image = Arc::new(image);
        state.connection = Some(connection.clone());
        state.inventory = Some(image.clone());
        Ok(MaterialInventoryCandidate {
            tracker: self.state.clone(),
            connection: connection.clone(),
            revision: state.revision,
            publication: state.publication,
            inventory: image,
        })
    }
    /// Trusted queue-success hook, not a UI/bare-bool completion. This makes only
    /// the captured quote source available and can never mark an operation Applied.
    pub(crate) fn publish_inventory(&self, candidate: MaterialInventoryCandidate) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Material source unavailable")?;
        if !Arc::ptr_eq(&self.state, &candidate.tracker)
            || state.exhausted
            || state.revision != candidate.revision
            || state.publication != candidate.publication
            || state
                .connection
                .as_ref()
                .is_none_or(|c| !c.same(&candidate.connection))
            || state.inventory.as_ref() != Some(&candidate.inventory)
        {
            return Err("Stale material inventory publication");
        }
        state.ready = true;
        Ok(())
    }
    pub(crate) fn invalidate(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.ready = false;
            let _ = state.advance_revision();
            let _ = state.next_publication();
        }
    }
    pub(crate) fn source_for_actual(
        &self,
        connection: &MaterialConnection,
        actual: &InventoryModel,
        unique_id: u64,
        quantity: u64,
    ) -> Result<MaterialSource> {
        if unique_id == 0 || quantity == 0 || quantity > u64::from(u32::MAX) {
            return Err("Invalid material source identity or quantity");
        }
        let image = inventory_image(actual)?;
        let state = self
            .state
            .lock()
            .map_err(|_| "Material source unavailable")?;
        if !state.ready
            || state.exhausted
            || state
                .connection
                .as_ref()
                .is_none_or(|c| !c.same(connection))
            || state.inventory.as_deref() != Some(&image)
        {
            return Err("Current complete material source required");
        }
        let selected = actual
            .items
            .iter()
            .find(|item| item.unique_id == Some(unique_id))
            .filter(|item| item.container == 0 && u64::from(item.quantity) >= quantity)
            .ok_or("Material source is not a positive-UID Bag root")?;
        Ok(MaterialSource {
            tracker: self.state.clone(),
            connection: connection.clone(),
            revision: state.revision,
            inventory: state
                .inventory
                .as_ref()
                .expect("verified source image")
                .clone(),
            unique_id,
            input_count: u64::from(selected.quantity),
            quantity,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum MaterialPhase {
    #[default]
    Ready,
    Quoting,
    Quoted,
    Pending,
    /// Only the private Runtime MaterialApplied completion boundary enters it.
    Complete,
    Rejected,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MaterialClientModel {
    pub phase: MaterialPhase,
    pub quote: Option<wire::Quote>,
    pub quote_id: Option<String>,
    pub receipt: Option<wire::Receipt>,
    pub visible: bool,
    pub pending: bool,
    pub reload_required: bool,
    pub notice: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct MaterialDispatch {
    pub owner: NativeCommandStamp,
    pub request: wire::ClientRequest,
    pub body: String,
    namespace: Arc<()>,
}

/// Preparation candidate for a future independent gameplay bundle. Even a
/// current candidate only attests the original received snapshot, not Applied.
#[derive(Clone)]
pub(crate) struct MaterialApplicationCandidate {
    connection: MaterialConnection,
    request_id: u64,
    quote_id: String,
    snapshot: Arc<Value>,
}
impl std::fmt::Debug for MaterialApplicationCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialApplicationCandidate")
            .field("request_id", &self.request_id)
            .field("quote_id", &self.quote_id)
            .finish_non_exhaustive()
    }
}
impl MaterialApplicationCandidate {
    pub(crate) fn owner(&self) -> NativeCommandStamp {
        self.connection.owner
    }
    pub(crate) fn request_id(&self) -> u64 {
        self.request_id
    }
    pub(crate) fn quote_id(&self) -> &str {
        &self.quote_id
    }
    pub(crate) fn snapshot(&self) -> &Value {
        &self.snapshot
    }
    fn same(&self, other: &Self) -> bool {
        self.connection.same(&other.connection)
            && self.request_id == other.request_id
            && self.quote_id == other.quote_id
            && Arc::ptr_eq(&self.snapshot, &other.snapshot)
    }
}
#[derive(Debug)]
pub(crate) struct MaterialReceived {
    pub frame: wire::ServerFrame,
    pub application_candidate: Option<MaterialApplicationCandidate>,
}

#[derive(Debug, Clone, Default)]
struct ActorState {
    model: MaterialClientModel,
    source: Option<MaterialSource>,
    confirmed: BTreeSet<String>,
    application: Option<MaterialApplicationCandidate>,
    // Last actual strict Quoted response, never UI-supplied recovery terms.
    // A terminal presentation can be pruned by reset before the UI consumes it,
    // so view reset/Close cannot establish that this recovery id is forgotten.
    recovery: Option<MaterialRecovery>,
}
#[derive(Debug, Clone)]
struct MaterialRecovery {
    quote: wire::Quote,
    receipt: Option<wire::Receipt>,
}
impl ActorState {
    fn reset_view(&mut self) {
        self.model = MaterialClientModel::default();
        self.source = None;
        self.application = None;
    }
}
#[derive(Debug)]
struct Control {
    connection: MaterialConnection,
    request: wire::ClientRequest,
    body: String,
    source: Option<MaterialSource>,
    deadline: u64,
    entered: bool,
}
#[derive(Debug, Default)]
pub(crate) struct NativeHuntingMaterialClient {
    binding: Option<MaterialConnection>,
    actors: BTreeMap<MaterialActor, ActorState>,
    last_request_id: u64,
    control: Option<Control>,
}

impl NativeHuntingMaterialClient {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn bind(&mut self, connection: MaterialConnection) -> Result<()> {
        if self
            .binding
            .as_ref()
            .is_some_and(|prior| prior.same(&connection))
        {
            return Ok(());
        }
        if !self.actors.contains_key(&connection.actor) && self.actors.len() >= MAX_ACTORS {
            self.withdraw();
            return Err("Material actor custody capacity reached");
        }
        self.withdraw();
        let state = self.actors.entry(connection.actor.clone()).or_default();
        if state.model.pending {
            state.source = None;
            state.application = None;
            state.model.visible = false;
            state.model.phase = MaterialPhase::Pending;
            // A newly verified full entry may query the original receipt; it
            // cannot reuse the old source or submit another confirmation.
            state.model.reload_required = false;
            state.model.notice = Some("originalQueryRequired".into());
        } else {
            state.reset_view();
        }
        self.binding = Some(connection);
        Ok(())
    }
    pub(crate) fn withdraw(&mut self) {
        self.control = None;
        if let Some(connection) = self.binding.take() {
            if let Some(state) = self.actors.get_mut(&connection.actor) {
                state.application = None;
                state.source = None;
                state.model.visible = false;
                if !state.model.pending {
                    state.reset_view();
                }
            }
        }
    }
    fn connection(&self, owner: NativeCommandStamp) -> Result<MaterialConnection> {
        self.binding
            .as_ref()
            .filter(|binding| binding.owner == owner)
            .cloned()
            .ok_or("Stale material owner")
    }
    fn prepare(
        &mut self,
        connection: MaterialConnection,
        action: wire::Action,
        source: Option<MaterialSource>,
        now_ms: u64,
    ) -> Result<MaterialDispatch> {
        if self.control.is_some() {
            return Err("Material control already outstanding");
        }
        let deadline = now_ms
            .checked_add(CONTROL_TIMEOUT_MS)
            .ok_or("Invalid material clock")?;
        let id = self
            .last_request_id
            .checked_add(1)
            .ok_or("Material request IDs exhausted")?;
        let request = wire::ClientRequest::new(U64::new(id), action)
            .map_err(|_| "Invalid material request")?;
        let body = wire::encode_client_request(&request).map_err(|_| "Material encoding failed")?;
        self.last_request_id = id;
        self.control = Some(Control {
            connection: connection.clone(),
            request: request.clone(),
            body: body.clone(),
            source,
            deadline,
            entered: false,
        });
        Ok(MaterialDispatch {
            owner: connection.owner,
            request,
            body,
            namespace: connection.namespace,
        })
    }
    pub(crate) fn prepare_quote(
        &mut self,
        source: &MaterialSource,
        now_ms: u64,
    ) -> Result<MaterialDispatch> {
        let connection = self.connection(source.owner())?;
        if !connection.same(&source.connection) || !source.current() {
            return Err("Current material source required");
        }
        let state = self
            .actors
            .get(&connection.actor)
            .ok_or("Material actor custody missing")?;
        if state.model.pending
            || state.model.reload_required
            || self.control.is_some()
            || state.confirmed.len() >= MAX_CONFIRMED_QUOTES
        {
            return Err("Material quote unavailable");
        }
        let dispatch = self.prepare(
            connection.clone(),
            wire::Action::Quote {
                unique_id: U64::new(source.unique_id),
                quantity: U64::new(source.quantity),
            },
            Some(source.clone()),
            now_ms,
        )?;
        let state = self
            .actors
            .get_mut(&connection.actor)
            .expect("current material actor");
        state.reset_view();
        state.source = Some(source.clone());
        state.model.visible = true;
        state.model.phase = MaterialPhase::Quoting;
        Ok(dispatch)
    }
    pub(crate) fn prepare_confirm(
        &mut self,
        owner: NativeCommandStamp,
        quote_id: &str,
        source: &MaterialSource,
        now_ms: u64,
    ) -> Result<MaterialDispatch> {
        let connection = self.connection(owner)?;
        let state = self
            .actors
            .get(&connection.actor)
            .ok_or("Material actor custody missing")?;
        let quote = state
            .model
            .quote
            .as_ref()
            .ok_or("Original material quote required")?;
        if !connection.same(&source.connection)
            || !source.current()
            || state
                .source
                .as_ref()
                .is_none_or(|original| !original.same(source))
            || state.model.pending
            || state.model.reload_required
            || !state.model.visible
            || state.model.phase != MaterialPhase::Quoted
            || quote.quote_id != quote_id
            || state.confirmed.contains(quote_id)
            || state.confirmed.len() >= MAX_CONFIRMED_QUOTES
            || now_ms >= quote.expires_at_ms as u64
        {
            return Err("Original material confirmation unavailable");
        }
        self.prepare(
            connection,
            wire::Action::Confirm {
                quote_id: quote_id.into(),
            },
            Some(source.clone()),
            now_ms,
        )
    }
    pub(crate) fn prepare_query(
        &mut self,
        owner: NativeCommandStamp,
        quote_id: &str,
        now_ms: u64,
    ) -> Result<MaterialDispatch> {
        let connection = self.connection(owner)?;
        let state = self
            .actors
            .get(&connection.actor)
            .ok_or("Material actor custody missing")?;
        if state.model.reload_required
            || state.model.pending && state.model.quote_id.as_deref() != Some(quote_id)
        {
            return Err("Original material query required");
        }
        let recovery = state
            .recovery
            .as_ref()
            .filter(|saved| saved.quote.quote_id == quote_id)
            .cloned()
            .ok_or("Privately retained material quote required")?;
        let dispatch = self.prepare(
            connection.clone(),
            wire::Action::Query {
                quote_id: quote_id.into(),
            },
            None,
            now_ms,
        )?;
        let state = self
            .actors
            .get_mut(&connection.actor)
            .expect("current material actor");
        // Restoring a forgotten view grants only original read-only Query.
        // Neither the old source nor a Confirm path is restored, including
        // after a prepared-but-unentered Confirm or a lost terminal delivery.
        state.model = MaterialClientModel {
            phase: MaterialPhase::Pending,
            quote: Some(recovery.quote.clone()),
            quote_id: Some(recovery.quote.quote_id),
            receipt: recovery.receipt,
            visible: true,
            pending: true,
            notice: Some("originalQueryRequired".into()),
            ..Default::default()
        };
        state.source = None;
        state.application = None;
        Ok(dispatch)
    }
    /// Preparation does not consume confirmation. The actual gateway calls this
    /// while holding its final physical owner fence, immediately before
    /// irreversible start_send. It must recheck OwnedGatewayCommand sequence,
    /// current connection identity/capability and source under that same lock.
    /// This method deliberately does not reacquire that external fence.
    pub(crate) fn enter(&mut self, dispatch: &MaterialDispatch, now_ms: u64) -> Result<()> {
        let connection = self.connection(dispatch.owner)?;
        let control = self.control.as_ref().ok_or("Material control missing")?;
        if control.entered
            || !control.connection.same(&connection)
            || !Arc::ptr_eq(&dispatch.namespace, &connection.namespace)
            || control.request != dispatch.request
            || control.body != dispatch.body
            || now_ms >= control.deadline
        {
            return Err("Material transmission custody mismatch");
        }
        let state = self
            .actors
            .get(&connection.actor)
            .ok_or("Material actor custody missing")?;
        if state.model.reload_required {
            return Err("Material owner requires reload");
        }
        match &control.request.action {
            wire::Action::Quote { .. } => {
                let source = control.source.as_ref().ok_or("Material source missing")?;
                if state.model.pending
                    || !state.model.visible
                    || !source.current()
                    || !source.connection.same(&connection)
                    || state
                        .source
                        .as_ref()
                        .is_none_or(|saved| !saved.same(source))
                {
                    return Err("Material source retired before send");
                }
            }
            wire::Action::Confirm { quote_id } => {
                let source = control.source.as_ref().ok_or("Material source missing")?;
                let quote = state
                    .model
                    .quote
                    .as_ref()
                    .ok_or("Original material quote required")?;
                if state.model.pending
                    || !state.model.visible
                    || state.model.phase != MaterialPhase::Quoted
                    || !source.current()
                    || !source.connection.same(&connection)
                    || state
                        .source
                        .as_ref()
                        .is_none_or(|saved| !saved.same(source))
                    || quote.quote_id != *quote_id
                    || now_ms >= quote.expires_at_ms as u64
                    || state.confirmed.contains(quote_id)
                    || state.confirmed.len() >= MAX_CONFIRMED_QUOTES
                {
                    return Err("Material confirmation retired before send");
                }
            }
            wire::Action::Query { quote_id } => {
                if state.model.quote_id.as_deref() != Some(quote_id) {
                    return Err("Original material query required");
                }
            }
        }
        let state = self
            .actors
            .get_mut(&connection.actor)
            .expect("current material actor");
        match &control.request.action {
            wire::Action::Confirm { quote_id } => {
                state.confirmed.insert(quote_id.clone());
            }
            wire::Action::Quote { .. } | wire::Action::Query { .. } => {}
        }
        if !matches!(&control.request.action, wire::Action::Quote { .. }) {
            state.model.pending = true;
            state.model.phase = MaterialPhase::Pending;
            state.model.notice = None;
        }
        self.control
            .as_mut()
            .expect("checked material control")
            .entered = true;
        Ok(())
    }
    pub(crate) fn definitely_unsent(&mut self, request_id: u64) -> bool {
        if !self.control.as_ref().is_some_and(|control| {
            control.request.request_id.get() == request_id && !control.entered
        }) {
            return false;
        }
        let control = self
            .control
            .take()
            .expect("checked unsent material control");
        let state = self
            .actors
            .get_mut(&control.connection.actor)
            .expect("material control actor");
        if matches!(&control.request.action, wire::Action::Quote { .. }) {
            state.reset_view();
        }
        state.model.notice = Some("definitelyUnsent".into());
        true
    }
    pub(crate) fn receive(
        &mut self,
        owner: NativeCommandStamp,
        raw: &str,
    ) -> Result<MaterialReceived> {
        let connection = self.connection(owner)?;
        let frame = wire::parse_server_frame(raw).map_err(|_| "Invalid material server frame")?;
        let control = self
            .control
            .as_ref()
            .ok_or("Material response custody missing")?;
        if !control.entered || !control.connection.same(&connection) {
            return Err("Stale material response owner");
        }
        frame
            .validate_for_request(&control.request)
            .map_err(|_| "Material response correlation mismatch")?;
        // Complete validation happens on a clone. A malformed financial/source
        // reply cannot erase the retained quote, receipt, control or candidate.
        let mut next = self
            .actors
            .get(&connection.actor)
            .ok_or("Material actor custody missing")?
            .clone();
        match (&control.request.action, &frame.reply) {
            (wire::Action::Quote { .. }, wire::Reply::Quoted { quote }) => {
                let source = control
                    .source
                    .as_ref()
                    .ok_or("Original material source missing")?;
                if !next.model.visible
                    || !source.current()
                    || next.source.as_ref().is_none_or(|saved| !saved.same(source))
                    || quote.input_count.get() != source.input_count
                    || quote.unique_id.get() != source.unique_id
                    || quote.quantity.get() != source.quantity
                    || next.confirmed.contains(&quote.quote_id)
                {
                    return Err("Material quote no longer matches original Bag source");
                }
                next.model.quote = Some(quote.clone());
                next.model.quote_id = Some(quote.quote_id.clone());
                next.model.phase = MaterialPhase::Quoted;
                next.model.notice = None;
                next.recovery = Some(MaterialRecovery {
                    quote: quote.clone(),
                    receipt: None,
                });
            }
            (wire::Action::Quote { .. }, wire::Reply::Rejected { code, .. }) => {
                next.model.phase = MaterialPhase::Rejected;
                next.model.notice = Some(rejection(*code).into());
            }
            (wire::Action::Quote { .. }, wire::Reply::Unknown { .. }) => {
                next.reset_view();
                next.model.notice = Some("quoteUnknown".into());
            }
            (
                _,
                wire::Reply::Committed {
                    receipt,
                    reload_required,
                    ..
                },
            ) => {
                if let Some(quote) = next.model.quote.as_ref() {
                    receipt
                        .validate_for_quote(quote)
                        .map_err(|_| "Immutable material quote terms changed")?;
                }
                if next.model.quote_id.as_deref() != Some(receipt.quote_id.as_str())
                    || next
                        .model
                        .receipt
                        .as_ref()
                        .is_some_and(|known| known != receipt)
                {
                    return Err("Immutable material receipt mismatch");
                }
                next.model.receipt = Some(receipt.clone());
                let known = next
                    .recovery
                    .as_mut()
                    .filter(|saved| saved.quote.quote_id == receipt.quote_id)
                    .ok_or("Private material recovery custody missing")?;
                if known.receipt.as_ref().is_some_and(|prior| prior != receipt) {
                    return Err("Immutable material recovery receipt mismatch");
                }
                known.receipt = Some(receipt.clone());
                next.model.pending = true;
                next.model.phase = MaterialPhase::Pending;
                next.model.reload_required = *reload_required;
                next.model.notice = Some(
                    if *reload_required {
                        "reloadRequired"
                    } else {
                        "snapshotRequired"
                    }
                    .into(),
                );
                next.application = if !*reload_required {
                    frame
                        .snapshot
                        .as_ref()
                        .map(|snapshot| MaterialApplicationCandidate {
                            connection: connection.clone(),
                            request_id: frame.request_id.get(),
                            quote_id: receipt.quote_id.clone(),
                            snapshot: Arc::new(snapshot.clone()),
                        })
                } else {
                    None
                };
            }
            (
                wire::Action::Query { .. },
                wire::Reply::Rejected {
                    code: wire::Rejection::NotSubmitted,
                    ..
                },
            ) => {
                if next.model.receipt.is_some() {
                    return Err("Known material receipt cannot be withdrawn");
                }
                next.model.pending = false;
                next.model.phase = MaterialPhase::Rejected;
                next.model.notice = Some("notSubmitted".into());
                next.source = None;
                next.application = None;
            }
            (_, wire::Reply::Rejected { code, .. }) => {
                next.model.pending = true;
                next.model.phase = MaterialPhase::Pending;
                next.model.notice = Some(rejection(*code).into());
                next.application = None;
            }
            (_, wire::Reply::Pending { .. } | wire::Reply::Unknown { .. }) => {
                next.model.pending = true;
                next.model.phase = MaterialPhase::Pending;
                next.model.notice = Some(
                    if matches!(&frame.reply, wire::Reply::Pending { .. }) {
                        "pending"
                    } else {
                        "unknown"
                    }
                    .into(),
                );
                next.application = None;
            }
            _ => return Err("Unexpected material response"),
        }
        let application_candidate = next.application.clone();
        self.actors.insert(connection.actor, next);
        self.control = None;
        Ok(MaterialReceived {
            frame,
            application_candidate,
        })
    }
    /// This only reports an outstanding preparation candidate. It neither
    /// constructs an Applied proof nor transitions the client to Complete.
    pub(crate) fn application_is_current(&self, candidate: &MaterialApplicationCandidate) -> bool {
        self.binding.as_ref().is_some_and(|connection| {
            connection.same(&candidate.connection)
                && self.actors.get(&connection.actor).is_some_and(|state| {
                    state.model.pending
                        && !state.model.reload_required
                        && state
                            .model
                            .receipt
                            .as_ref()
                            .is_some_and(|r| r.quote_id == candidate.quote_id)
                        && state
                            .application
                            .as_ref()
                            .is_some_and(|saved| saved.same(candidate))
                })
        })
    }
    /// The physical host holds its current owner fence outside this call.
    /// Reauthorize the private proof here, including after it was popped, so a
    /// retired gate or a newer prepare can never release the Pending lane.
    /// There is no queue admission or gate reentry inside this callback.
    pub(super) fn applied(
        &mut self,
        candidate: &MaterialApplicationCandidate,
        gate: &mir2_bevy_runtime::hunting_material_economy::NativeHuntingMaterialGate,
        applied: &mir2_bevy_runtime::hunting_material_economy::NativeHuntingMaterialApplied,
    ) -> bool {
        gate.with_current_applied(applied, || {
            let owner = candidate.connection.owner;
            let epoch = applied.epoch();
            if !self.application_is_current(candidate)
                || candidate.request_id != applied.request_id()
                || candidate.quote_id != applied.quote_id()
                || candidate.connection.actor.realm() != applied.owner().realm()
                || candidate.connection.actor.account_id() != applied.owner().account()
                || candidate.connection.actor.character_index() != applied.owner().character_index()
                || !owner.matches_material_epoch(epoch)
            {
                return false;
            }
            let state = self
                .actors
                .get_mut(&candidate.connection.actor)
                .expect("current material actor");
            if state.model.quote.as_ref() != Some(applied.quote())
                || state.model.receipt.as_ref() != Some(applied.receipt())
            {
                return false;
            }
            state.model.pending = false;
            state.model.phase = MaterialPhase::Complete;
            state.model.notice = Some("committed".into());
            state.source = None;
            state.application = None;
            true
        })
        .unwrap_or(false)
    }
    pub(crate) fn check_timeout(&mut self, now_ms: u64) -> bool {
        if !self
            .control
            .as_ref()
            .is_some_and(|control| now_ms >= control.deadline)
        {
            return false;
        }
        let control = self.control.take().expect("expired material control");
        let state = self
            .actors
            .get_mut(&control.connection.actor)
            .expect("material control actor");
        state.model.phase = if state.model.pending {
            MaterialPhase::Pending
        } else if state.model.quote.is_some() {
            MaterialPhase::Quoted
        } else {
            MaterialPhase::Ready
        };
        state.model.notice = Some("timeout".into());
        true
    }
    pub(crate) fn close_view(&mut self, owner: NativeCommandStamp) -> bool {
        let Ok(connection) = self.connection(owner) else {
            return false;
        };
        let state = self
            .actors
            .get_mut(&connection.actor)
            .expect("current material actor");
        state.model.visible = false;
        if !state.model.pending {
            state.reset_view();
            self.control = None;
        }
        true
    }
    pub(crate) fn model(&self) -> MaterialClientModel {
        self.binding
            .as_ref()
            .and_then(|connection| self.actors.get(&connection.actor))
            .map(|state| state.model.clone())
            .unwrap_or_default()
    }
}

fn rejection(code: wire::Rejection) -> &'static str {
    match code {
        wire::Rejection::Disabled => "disabled",
        wire::Rejection::Unauthenticated => "unauthenticated",
        wire::Rejection::Unavailable => "unavailable",
        wire::Rejection::InvalidRequest => "invalidRequest",
        wire::Rejection::Unsupported => "unsupported",
        wire::Rejection::Expired => "expired",
        wire::Rejection::ItemChanged => "itemChanged",
        wire::Rejection::NotSubmitted => "notSubmitted",
        wire::Rejection::Unconfigured => "unconfigured",
        wire::Rejection::InsufficientPermits => "insufficientPermits",
        wire::Rejection::InsufficientUnboundGold => "insufficientUnboundGold",
    }
}

#[cfg(test)]
#[path = "hunting_material_client_tests.rs"]
mod tests;
