//! Independent read-only NPC shop ingress. The existing Page/Core dispatcher
//! owns purchase attempts and transport; renderer actions are fenced intents.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use mir2_client_bevy::{
    inventory::InventoryModel, read_model::PlayerStats, shop::ShopModel,
    npc_gold_buy_attempt::{NpcGoldBuyAttemptPhase, NpcGoldBuyFeedback},
    npc_shop_buy::{plan_npc_gold_buy_json, valid_npc_gold_buy_inventory, NpcGoldBuyBlockReason, NpcGoldBuyPlan},
    npc_shop_ui::ordinary_gold_surface,
    portable_npc_shop_ui::{NpcShopSurfaceContext, NpcShopSurfaceIdentity,
        NpcShopSurfacePresentation, NpcShopSurfaceReadModel, NpcShopSurfaceProof,
        NpcShopSurfaceGesture, NpcShopSurfaceIntent, NpcShopSurfacePointerEdge,
        NpcShopSurfacePointerPhase, NpcShopSurfacePointerOrigin, NpcShopSurfaceState},
};
const SAFE: u64 = 9_007_199_254_740_991;

mod optional_decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
        if *value == Some(0) { return Err(serde::ser::Error::custom("zero NPC decimal revision")); }
        match value { Some(value) => s.serialize_some(&value.to_string()), None => s.serialize_none() }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
        let text = Option::<String>::deserialize(d)?;
        text.map(|text| {
            mir2_client_bevy::portable_npc_shop_ui::decimal_u64::parse(&text)
                .ok_or_else(|| serde::de::Error::custom("invalid NPC decimal revision"))
        }).transpose()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum NpcShopFeedbackPhase { Queued, Bound, Entered, Flushed, Unknown, DefinitelyUnsent }
impl NpcShopFeedbackPhase {
    fn to_common(self) -> NpcGoldBuyAttemptPhase {
        match self {
            Self::Queued => NpcGoldBuyAttemptPhase::Queued, Self::Bound => NpcGoldBuyAttemptPhase::Bound,
            Self::Entered => NpcGoldBuyAttemptPhase::Entered, Self::Flushed => NpcGoldBuyAttemptPhase::Flushed,
            Self::Unknown => NpcGoldBuyAttemptPhase::Unknown,
            Self::DefinitelyUnsent => NpcGoldBuyAttemptPhase::DefinitelyUnsent,
        }
    }
    fn from_common(phase: NpcGoldBuyAttemptPhase) -> Self {
        match phase {
            NpcGoldBuyAttemptPhase::Queued => Self::Queued, NpcGoldBuyAttemptPhase::Bound => Self::Bound,
            NpcGoldBuyAttemptPhase::Entered => Self::Entered, NpcGoldBuyAttemptPhase::Flushed => Self::Flushed,
            NpcGoldBuyAttemptPhase::Unknown => Self::Unknown,
            NpcGoldBuyAttemptPhase::DefinitelyUnsent => Self::DefinitelyUnsent,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NpcShopFeedback {
    phase: Option<NpcShopFeedbackPhase>, pending: bool, can_reserve: bool, previous_unknown: u32,
}
impl NpcShopFeedback {
    fn valid(self) -> bool {
        self.previous_unknown <= 32 && !(self.pending && self.can_reserve)
            && (!self.pending || self.phase.is_some_and(|p| p != NpcShopFeedbackPhase::DefinitelyUnsent))
    }
    fn to_common(self) -> NpcGoldBuyFeedback {
        NpcGoldBuyFeedback { phase: self.phase.map(NpcShopFeedbackPhase::to_common),
            pending: self.pending, can_reserve: self.can_reserve, previous_unknown: self.previous_unknown as usize }
    }
    fn from_common(feedback: NpcGoldBuyFeedback) -> Self {
        Self { phase: feedback.phase.map(NpcShopFeedbackPhase::from_common), pending: feedback.pending,
            can_reserve: feedback.can_reserve, previous_unknown: feedback.previous_unknown.min(32) as u32 }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpcShopSnapshot {
    #[serde(flatten)] identity: NpcShopSurfaceIdentity,
    revision: u64, model_revision: u64, presentation_revision: u64,
    service_revision: u64, catalog_revision: u64,
    #[serde(with = "optional_decimal")] core_authority_revision: Option<u64>,
    open: bool, input_enabled: bool, show_buy: bool,
    presentation: Option<NpcShopSurfacePresentation>,
    shop: ShopModel, inventory: InventoryModel, player: PlayerStats,
    language: String, feedback: NpcShopFeedback,
}

// Reject duplicate members and excessive nesting before any legacy serde
// defaults can manufacture a supposedly complete shop or item carrier.
struct StrictJson(usize);
impl<'de> serde::de::DeserializeSeed<'de> for StrictJson {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if self.0 > 32 { return Err(serde::de::Error::custom("nested NPC JSON")); }
        d.deserialize_any(self)
    }
}
impl<'de> serde::de::Visitor<'de> for StrictJson {
    type Value = Value;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("bounded NPC JSON") }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> { Ok(v.into()) }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> { Ok(v.into()) }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> { Ok(v.into()) }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(v).map(Value::Number).ok_or_else(|| E::custom("nonfinite NPC number"))
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> { Ok(Value::String(v.into())) }
    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> { Ok(Value::String(v)) }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> { Ok(Value::Null) }
    fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> { Ok(Value::Null) }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(StrictJson(self.0 + 1))? {
            if values.len() >= 2048 { return Err(serde::de::Error::custom("long NPC array")); }
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() > 256 || values.len() >= 256 || values.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate or excessive NPC member"));
            }
            values.insert(key, map.next_value_seed(StrictJson(self.0 + 1))?);
        }
        Ok(Value::Object(values))
    }
}
fn strict(json: &str) -> Result<Value, &'static str> {
    use serde::de::DeserializeSeed;
    if json.len() > 1_048_576 { return Err("NPC JSON too large"); }
    let mut d = serde_json::Deserializer::from_str(json);
    let value = StrictJson(0).deserialize(&mut d).map_err(|_| "invalid NPC JSON")?;
    d.end().map_err(|_| "invalid NPC JSON")?;
    Ok(value)
}
fn required(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object| keys.iter().all(|key| object.contains_key(*key)))
}
fn exact_keys(value: &Value, keys: &[&str]) -> bool {
    required(value, keys) && value.as_object().is_some_and(|object| object.len() == keys.len())
}
fn parse_identity(json: &str) -> Result<NpcShopSurfaceIdentity, &'static str> {
    let value = strict(json)?;
    if !exact_keys(&value, &["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId"]) {
        return Err("invalid NPC withdrawal fields");
    }
    let identity: NpcShopSurfaceIdentity = serde_json::from_value(value).map_err(|_| "invalid NPC withdrawal")?;
    identity.valid().then_some(identity).ok_or("invalid NPC withdrawal identity")
}
fn parse_pointer(json: &str) -> Result<NpcShopSurfacePointerEdge, &'static str> {
    let value = strict(json)?;
    if !exact_keys(&value, &["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId",
        "revision", "modelRevision", "presentationRevision", "serviceRevision", "catalogRevision",
        "coreAuthorityRevision", "controlRevision", "pointerId", "downSequence", "sequence", "origin", "button", "phase", "x", "y"]) {
        return Err("invalid NPC pointer fields");
    }
    let edge: NpcShopSurfacePointerEdge = serde_json::from_value(value).map_err(|_| "invalid NPC pointer")?;
    edge.valid().then_some(edge).ok_or("invalid NPC pointer identity/gesture")
}
fn catalog_icon_paths(shop: &ShopModel) -> Result<Vec<String>, &'static str> {
    let mut paths = Vec::new();
    for good in &shop.goods {
        let index = good.user_item_image_index().ok_or("NPC shop item icon metadata unavailable")?;
        paths.push(format!("original-ui/Items/{index}.png"));
    }
    paths.sort(); paths.dedup(); Ok(paths)
}
impl NpcShopSnapshot {
    fn parse(json: &str) -> Result<Self, &'static str> {
        let value = strict(json)?;
        if !exact_keys(&value, &["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision",
            "playerObjectId", "revision", "modelRevision", "presentationRevision", "serviceRevision",
            "catalogRevision", "coreAuthorityRevision", "open", "inputEnabled", "showBuy", "presentation",
            "shop", "inventory", "player", "language", "feedback"])
            || !required(&value["feedback"], &["phase", "pending", "canReserve", "previousUnknown"])
            || !required(&value["player"], &["hp", "maxHp", "mp", "maxMp", "gold", "credit", "crystalStats",
                "level", "experience", "maxExperience", "currentWeight", "currentWeightKnown", "weights",
                "maxWeight", "name", "className", "gender", "hair", "wingEffect", "guildName", "guildRankName",
                "mapName", "inSafeZone"]) { return Err("incomplete NPC snapshot"); }
        // Send the ORIGINAL raw models to the existing complete-carrier oracle.
        // Structurally complete models may still be blocked by the real planner.
        let raw = serde_json::json!({"shop":value["shop"],"inventory":value["inventory"],"quantity":1});
        let plan: NpcGoldBuyPlan = serde_json::from_str(&plan_npc_gold_buy_json(&raw.to_string()))
            .map_err(|_| "invalid NPC model oracle")?;
        if plan.block_reason == Some(NpcGoldBuyBlockReason::InvalidInput) { return Err("incomplete NPC raw models"); }
        let snapshot: Self = serde_json::from_value(value).map_err(|_| "invalid NPC snapshot fields")?;
        snapshot.validate()?;
        Ok(snapshot)
    }
    fn context(&self) -> NpcShopSurfaceContext {
        NpcShopSurfaceContext { identity: self.identity, revision: self.revision,
            model_revision: self.model_revision, presentation_revision: self.presentation_revision,
            service_revision: self.service_revision, catalog_revision: self.catalog_revision,
            core_authority_revision: self.core_authority_revision, open: self.open,
            input_enabled: self.input_enabled, show_buy: self.show_buy, ready: false,
            presentation: self.presentation, language: self.language.clone(), feedback: self.feedback.to_common() }
    }
    fn validate(&self) -> Result<(), &'static str> {
        if !self.context().valid() || !self.feedback.valid()
            || self.open && self.presentation.is_none()
            || !matches!(self.language.as_str(), "en" | "zh-CN" | "ja") {
            return Err("invalid NPC identity/presentation/feedback");
        }
        if self.open && !ordinary_gold_surface(&self.shop, self.show_buy)
            || self.input_enabled && self.core_authority_revision.is_none() { return Err("NPC directory needs legacy UI"); }
        if self.inventory.capacity != InventoryModel::canonical_capacity(self.inventory.capacity) {
            return Err("invalid NPC inventory capacity");
        }
        if self.shop.goods.len() > 2048 || self.inventory.items.len() > 160 { return Err("NPC model array too large"); }
        let mut ids = HashSet::new();
        for good in &self.shop.goods {
            if good.unique_id > SAFE || !ids.insert(good.unique_id) || good.name.len() > 256
                || good.description.len() > 8192 || good.icon_width > 16384 || good.icon_height > 16384 {
                return Err("invalid NPC catalog content");
            }
        }
        if !valid_npc_gold_buy_inventory(&self.inventory)
            || self.inventory.npc_gold_trade_capacity.as_ref().is_some_and(|e| e.fresh_compatible_unique_ids.iter().any(|id| *id > SAFE)) {
            return Err("invalid NPC inventory roster");
        }
        for item in &self.inventory.items {
            if item.key.len() > 256 || item.name.len() > 256 || item.description.len() > 8192
                || item.unique_id.is_some_and(|id| id > SAFE) {
                return Err("invalid NPC inventory content");
            }
        }
        if self.player.name.as_ref().is_some_and(|name| name.len() > 256) { return Err("invalid NPC player name"); }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct NpcShopSource {
    identity: NpcShopSurfaceIdentity, revision: u64, model_revision: u64,
    presentation_revision: u64, service_revision: u64, catalog_revision: u64,
    core_authority_revision: Option<u64>,
}
impl NpcShopSource {
    fn matches(self, proof: NpcShopSurfaceProof) -> bool {
        self.identity == proof.identity && self.revision == proof.revision && self.model_revision == proof.model_revision
            && self.presentation_revision == proof.presentation_revision && self.service_revision == proof.service_revision
            && self.catalog_revision == proof.catalog_revision && self.core_authority_revision == Some(proof.core_authority_revision)
    }
}
#[derive(Default)]
struct NpcShopIngress {
    accepted: Option<NpcShopSource>, pending: Option<NpcShopSnapshot>, withdrawn: Option<NpcShopSurfaceIdentity>,
    edges: Vec<NpcShopSurfacePointerEdge>, lease: Option<NpcShopSurfacePointerEdge>, last_edge_sequence: u64,
    cancel_pointer: bool, error: Option<String>, generation: u64, exhausted: bool,
}
impl NpcShopIngress {
    fn bump(&mut self) -> bool {
        if self.exhausted { return false; }
        if let Some(next) = self.generation.checked_add(1) { self.generation = next; true }
        else { self.exhausted = true; self.pending = None; self.edges.clear(); self.lease = None;
            self.cancel_pointer = true; self.error = Some("NPC ingress clock exhausted".into()); false }
    }
    fn accept(&mut self, s: NpcShopSnapshot) -> bool {
        if s.validate().is_err() { return false; }
        if let Some(old) = self.accepted {
            if !s.identity.can_follow(old.identity) || s.identity.run_generation == old.identity.run_generation
                && (s.revision <= old.revision || s.model_revision < old.model_revision
                    || s.presentation_revision < old.presentation_revision || s.service_revision < old.service_revision
                    || s.catalog_revision < old.catalog_revision
                    || s.core_authority_revision.zip(old.core_authority_revision).is_some_and(|(new, old)| new < old)) {
                return false;
            }
        }
        if !self.bump() { return false; }
        if self.accepted.is_none_or(|old| old.identity != s.identity) { self.last_edge_sequence = 0; }
        self.accepted = Some(NpcShopSource { identity: s.identity, revision: s.revision, model_revision: s.model_revision,
            presentation_revision: s.presentation_revision, service_revision: s.service_revision,
            catalog_revision: s.catalog_revision, core_authority_revision: s.core_authority_revision });
        self.pending = Some(s); self.withdrawn = None; self.edges.clear(); self.lease = None;
        self.cancel_pointer = true; self.error = None; true
    }
    fn withdraw(&mut self, identity: NpcShopSurfaceIdentity) -> bool {
        if self.accepted.is_none_or(|s| s.identity != identity) || !self.bump() { return false; }
        self.pending = None; self.withdrawn = Some(identity); self.edges.clear(); self.lease = None;
        self.cancel_pointer = true; self.error = None; true
    }
    fn cancel(&mut self) {
        self.bump(); self.edges.clear(); self.lease = None; self.cancel_pointer = true;
    }
    fn reject_current(&mut self, json: &str, error: &'static str) {
        // An old owner's malformed snapshot is not allowed to disable a new one.
        let identity = strict(json).ok().and_then(|v| Some(NpcShopSurfaceIdentity {
            run_generation: v["runGeneration"].as_u64()?, connection_generation: v["connectionGeneration"].as_u64()?,
            session_generation: v["sessionGeneration"].as_u64()?, owner_revision: v["ownerRevision"].as_u64()?,
            player_object_id: u32::try_from(v["playerObjectId"].as_u64()?).ok()?,
        }));
        if identity.is_some_and(|id| id.valid() && self.accepted.is_some_and(|s| s.identity == id)) {
            self.cancel(); self.pending = None; self.error = Some(error.into());
        }
    }
    fn settled(&self) -> bool {
        !self.exhausted && self.pending.is_none() && self.withdrawn.is_none() && self.error.is_none() && !self.cancel_pointer
    }
    fn pointer(&mut self, edge: NpcShopSurfacePointerEdge, current: bool) -> bool {
        if !self.settled() || !edge.valid() || self.accepted.is_none_or(|s| !s.matches(edge.proof))
            || edge.gesture.sequence <= self.last_edge_sequence || self.edges.len() >= 256 { return false; }
        let cleanup = matches!(edge.phase, NpcShopSurfacePointerPhase::Up | NpcShopSurfacePointerPhase::Cancel | NpcShopSurfacePointerPhase::Blur);
        if !current && !cleanup { return false; }
        if edge.phase == NpcShopSurfacePointerPhase::Down {
            if self.lease.is_some() { return false; }
        } else if !self.lease.is_some_and(|down| edge.matches_lease(down)) { return false; }
        if !self.bump() { return false; }
        self.lease = if cleanup { None } else { Some(edge) };
        self.last_edge_sequence = edge.gesture.sequence; self.edges.push(edge); true
    }
    fn checkpoint(&self, intent: &NpcShopSurfaceIntent, status: &NpcShopStatus, sink_generation: u64)
        -> Option<NpcShopForwardCheckpoint> {
        if !self.settled() || sink_generation == 0 || !status.ready || !status.input_enabled
            || status.proof() != Some(intent.proof) || self.accepted.is_none_or(|s| !s.matches(intent.proof))
            || intent.gesture.origin != NpcShopSurfacePointerOrigin::Shop || intent.gesture.button != 0
            || intent.gesture.sequence != self.last_edge_sequence { return None; }
        Some(NpcShopForwardCheckpoint { ingress_generation: self.generation, sink_generation,
            proof: intent.proof, gesture: intent.gesture, intent_sequence: intent.intent_sequence })
    }
    fn checkpoint_current(&self, checkpoint: NpcShopForwardCheckpoint, intent: &NpcShopSurfaceIntent,
        status: &NpcShopStatus, sink_generation: u64) -> bool {
        checkpoint.ingress_generation == self.generation && checkpoint.sink_generation == sink_generation
            && checkpoint.proof == intent.proof && checkpoint.gesture == intent.gesture
            && checkpoint.intent_sequence == intent.intent_sequence && self.checkpoint(intent, status, sink_generation).is_some()
    }
}
#[derive(Debug, Clone, Copy)]
struct NpcShopForwardCheckpoint {
    ingress_generation: u64, sink_generation: u64, proof: NpcShopSurfaceProof,
    gesture: NpcShopSurfaceGesture, intent_sequence: u64,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct InputRegion { left: f32, top: f32, width: f32, height: f32 }
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct NpcShopStatus {
    #[serde(flatten)] identity: NpcShopSurfaceIdentity,
    frame: u64, ready: bool, input_enabled: bool,
    applied_revision: u64, applied_model_revision: u64, applied_presentation_revision: u64,
    applied_service_revision: u64, applied_catalog_revision: u64,
    #[serde(with = "optional_decimal")] core_authority_revision: Option<u64>,
    #[serde(with = "optional_decimal")] control_revision: Option<u64>,
    selected_id: Option<u64>, quantity: u16, start_index: usize, feedback: NpcShopFeedback,
    input_regions: Vec<InputRegion>, error: Option<String>,
}
impl NpcShopStatus {
    fn from_context(c: &NpcShopSurfaceContext, state: &NpcShopSurfaceState, read: &NpcShopSurfaceReadModel) -> Self {
        let view = state.view(c, read);
        Self { identity: c.identity, ready: c.ready, input_enabled: c.ready && c.input_enabled,
            applied_revision: c.revision, applied_model_revision: c.model_revision,
            applied_presentation_revision: c.presentation_revision, applied_service_revision: c.service_revision,
            applied_catalog_revision: c.catalog_revision, core_authority_revision: c.core_authority_revision,
            control_revision: view.stamp.map(|stamp| stamp.presentation_revision),
            selected_id: view.selected_id, quantity: view.quantity, start_index: view.start_index,
            feedback: NpcShopFeedback::from_common(c.feedback), ..Default::default() }
    }
    fn proof(&self) -> Option<NpcShopSurfaceProof> {
        Some(NpcShopSurfaceProof { identity: self.identity, revision: self.applied_revision,
            model_revision: self.applied_model_revision, presentation_revision: self.applied_presentation_revision,
            service_revision: self.applied_service_revision, catalog_revision: self.applied_catalog_revision,
            core_authority_revision: self.core_authority_revision?, control_revision: self.control_revision? })
    }
    fn disable(&mut self) { self.ready = false; self.input_enabled = false; self.input_regions.clear(); }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use bevy::{prelude::*, asset::LoadState, ui::UiSystems};
    use js_sys::Function;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    use mir2_client_bevy::{
        portable_npc_shop_ui::{Mir2PortableNpcShopUiPlugin, NpcShopSurfacePointerEdges,
            NpcShopSurfaceIntentQueue, NpcShopSurfaceTreeObserver},
        portable_quest_ui::QuestUiFont, crystal_ui::{hud::SharedHudSurface, shop_paint::SHOP_REQUIRED_SKINS},
    };
    const FONT: &str = "original-ui/fonts/NotoSansCJKsc-Regular.otf";
    #[derive(Default)]
    struct Sink { function: Option<Function>, generation: u64, exhausted: bool }
    impl Sink {
        fn replace(&mut self, function: Option<Function>) {
            if !self.exhausted {
                if let Some(next) = self.generation.checked_add(1) { self.generation = next; self.function = function; return; }
            }
            self.exhausted = true; self.function = None;
        }
    }
    thread_local! {
        static INGRESS: RefCell<NpcShopIngress> = RefCell::new(NpcShopIngress::default());
        static STATUS: RefCell<NpcShopStatus> = RefCell::new(NpcShopStatus::default());
        static SINK: RefCell<Sink> = RefCell::new(Sink::default());
    }
    #[derive(Resource, Default)]
    struct Applied {
        snapshot: Option<NpcShopSnapshot>, font: Option<Handle<Font>>, skins: Vec<Handle<Image>>,
        icons: Vec<Handle<Image>>, paths: Vec<String>, asset_error: Option<String>,
    }
    pub(crate) fn install(app: &mut App) {
        app.init_resource::<Applied>().add_plugins(Mir2PortableNpcShopUiPlugin)
            .add_systems(Update, ingest.in_set(mir2_client_bevy::pending_operations::PendingLifecycleSet::Ingest)
                .after(crate::quest_ui_host::QuestHostIngestSet))
            .add_systems(PostUpdate, publish.after(UiSystems::Layout)
                .after(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate))
            .add_systems(Last, forward);
    }
    fn ingest(mut applied: ResMut<Applied>, mut c: ResMut<NpcShopSurfaceContext>,
        mut read: ResMut<NpcShopSurfaceReadModel>, mut state: ResMut<NpcShopSurfaceState>,
        mut edges: ResMut<NpcShopSurfacePointerEdges>, mut queue: ResMut<NpcShopSurfaceIntentQueue>,
        server: Res<AssetServer>, mut font: ResMut<QuestUiFont>) {
        let (snapshot, withdrawn, error, cancel) = INGRESS.with(|i| { let mut i = i.borrow_mut();
            (i.pending.take(), i.withdrawn.take(), i.error.clone(), std::mem::take(&mut i.cancel_pointer)) });
        if cancel { state.cancel_pointer(); queue.clear(); edges.0.clear(); }
        if error.is_some() || withdrawn.is_some_and(|id| id == c.identity) {
            c.open = false; c.input_enabled = false; c.ready = false;
            state.invalidate(); queue.clear(); edges.0.clear(); applied.snapshot = None; applied.asset_error = None;
        }
        if let Some(s) = snapshot {
            state.cancel_pointer(); queue.clear(); edges.0.clear(); *c = s.context();
            *read = NpcShopSurfaceReadModel { shop: s.shop.clone(), inventory: s.inventory.clone(), player: s.player.clone() };
            if s.open && applied.font.is_none() {
                let handle = server.load::<Font>(FONT); font.0 = handle.clone(); applied.font = Some(handle);
                applied.skins = SHOP_REQUIRED_SKINS.iter().map(|path| server.load::<Image>(*path)).collect();
            }
            let (paths, asset_error) = match catalog_icon_paths(&s.shop) {
                Ok(paths) => (paths, None), Err(error) => (Vec::new(), Some(error.to_owned())),
            };
            if paths != applied.paths { applied.icons = paths.iter().map(|p| server.load::<Image>(p.clone())).collect(); applied.paths = paths; }
            applied.asset_error = asset_error;
            applied.snapshot = Some(s);
        }
        edges.0.extend(INGRESS.with(|i| std::mem::take(&mut i.borrow_mut().edges)));
    }
    fn publish(mut c: ResMut<NpcShopSurfaceContext>, applied: Res<Applied>, state: Res<NpcShopSurfaceState>,
        read: Res<NpcShopSurfaceReadModel>, surface: Res<SharedHudSurface>, server: Res<AssetServer>,
        windows: Query<&Window>, tree: NpcShopSurfaceTreeObserver) {
        let observation = tree.observe(&c, &state, &read);
        let window = c.presentation.zip(windows.get(surface.window).ok())
            .is_some_and(|(p, w)| w.visible && p.matches_window(w.width(), w.height()));
        let mut loaded = applied.font.is_some(); let mut failed = false;
        for load in applied.font.iter().map(|h| server.get_load_state(h.id()))
            .chain(applied.skins.iter().map(|h| server.get_load_state(h.id())))
            .chain(applied.icons.iter().map(|h| server.get_load_state(h.id()))) {
            match load { Some(LoadState::Loaded) => {}, Some(LoadState::Failed(_)) => { failed = true; loaded = false; }, _ => loaded = false }
        }
        let settled = INGRESS.with(|i| i.borrow().settled());
        let sink = SINK.with(|s| s.borrow().function.is_some());
        let error = INGRESS.with(|i| i.borrow().error.clone()).or_else(|| applied.asset_error.clone())
            .or_else(|| state.error.map(str::to_owned))
            .or_else(|| failed.then(|| "NPC shop assets failed to load".into()))
            .or_else(|| (c.open && !window).then(|| "NPC shop presentation does not match window".into()));
        c.ready = applied.snapshot.is_some() && c.open && window && surface.active && loaded && !failed
            && observation.complete && sink && settled && error.is_none();
        let mut next = NpcShopStatus::from_context(&c, &state, &read); next.error = error;
        if c.ready { next.input_regions = observation.input_regions.into_iter().map(|r| InputRegion {
            left: r.min.x, top: r.min.y, width: r.width(), height: r.height() }).collect(); }
        STATUS.with(|s| { let mut s = s.borrow_mut(); next.frame = s.frame.saturating_add(1).min(SAFE); *s = next; });
    }
    fn forward(mut queue: ResMut<NpcShopSurfaceIntentQueue>, mut state: ResMut<NpcShopSurfaceState>,
        mut c: ResMut<NpcShopSurfaceContext>, read: Res<NpcShopSurfaceReadModel>) {
        for intent in queue.drain() {
            let status = STATUS.with(|s| s.borrow().clone());
            let (sink, generation) = SINK.with(|s| { let s = s.borrow(); (s.function.clone(), s.generation) });
            let checkpoint = INGRESS.with(|i| i.borrow().checkpoint(&intent, &status, generation));
            let accepted = state.allows(&intent, &c, &read) && checkpoint.is_some() && sink.as_ref().is_some_and(|sink| {
                let Ok(json) = serde_json::to_string(&intent) else { return false; };
                // All ingress/sink RefCell borrows have ended before JS reentry.
                // Non-Buy sink must withdraw exact Core preentry before returning true.
                sink.call1(&JsValue::NULL, &JsValue::from_str(&json)).ok().and_then(|v| v.as_bool()) == Some(true)
            });
            let current = accepted && checkpoint.is_some_and(|checkpoint| {
                let status = STATUS.with(|s| s.borrow().clone());
                let generation = SINK.with(|s| s.borrow().generation);
                INGRESS.with(|i| i.borrow().checkpoint_current(checkpoint, &intent, &status, generation))
                    && state.allows(&intent, &c, &read)
            });
            if current && state.apply_accepted(&intent, &c, &read).is_some() {
                c.ready = false; STATUS.with(|s| s.borrow_mut().disable());
                INGRESS.with(|i| i.borrow_mut().cancel());
            } else { state.finish_rejected(&intent); }
        }
    }
    #[wasm_bindgen(js_name = setMir2NpcShopUiSnapshot)]
    pub fn set_snapshot(json: String) -> bool {
        let snapshot = match NpcShopSnapshot::parse(&json) { Ok(s) => s, Err(error) => {
            INGRESS.with(|i| i.borrow_mut().reject_current(&json, error));
            let error = INGRESS.with(|i| i.borrow().error.clone());
            if error.is_some() { STATUS.with(|s| { let mut s = s.borrow_mut(); s.disable(); s.error = error; }); }
            return false;
        }};
        let accepted = INGRESS.with(|i| i.borrow_mut().accept(snapshot));
        if accepted { STATUS.with(|s| s.borrow_mut().disable()); } accepted
    }
    #[wasm_bindgen(js_name = getMir2NpcShopUiStatus)]
    pub fn get_status() -> String { STATUS.with(|s| serde_json::to_string(&*s.borrow()).expect("bounded NPC status")) }
    #[wasm_bindgen(js_name = setMir2NpcShopUiIntentSink)]
    pub fn set_sink(function: Function) {
        SINK.with(|s| s.borrow_mut().replace(Some(function)));
        INGRESS.with(|i| i.borrow_mut().cancel()); STATUS.with(|s| s.borrow_mut().disable());
    }
    #[wasm_bindgen(js_name = clearMir2NpcShopUiIntentSink)]
    pub fn clear_sink() {
        SINK.with(|s| s.borrow_mut().replace(None));
        INGRESS.with(|i| i.borrow_mut().cancel()); STATUS.with(|s| s.borrow_mut().disable());
    }
    #[wasm_bindgen(js_name = withdrawMir2NpcShopUiSnapshot)]
    pub fn withdraw(json: String) -> bool {
        let Ok(identity) = parse_identity(&json) else { return false; };
        let retired = INGRESS.with(|i| i.borrow_mut().withdraw(identity));
        if retired { STATUS.with(|s| s.borrow_mut().disable()); } retired
    }
    #[wasm_bindgen(js_name = setMir2NpcShopUiPointerEdge)]
    pub fn pointer(json: String) -> bool {
        let Ok(edge) = parse_pointer(&json) else { return false; };
        let current = STATUS.with(|s| { let s = s.borrow(); s.ready && s.input_enabled && s.proof() == Some(edge.proof) });
        INGRESS.with(|i| i.borrow_mut().pointer(edge, current))
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) use web::install;

#[cfg(test)]
#[path = "npc_shop_ui_host_tests.rs"]
mod tests;
