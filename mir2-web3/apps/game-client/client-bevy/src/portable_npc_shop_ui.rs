//! Portable ordinary Gold NPC shop surface. Controls enqueue immutable proofs;
//! only the runtime's accepted synchronous callback may apply local changes.
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiTargetCamera};
use serde::{Deserialize, Serialize};
use crate::inventory::InventoryModel;
use crate::read_model::PlayerStats;
use crate::shop::ShopModel;
use crate::npc_gold_buy_attempt::NpcGoldBuyFeedback;
use crate::npc_shop_buy::NpcGoldBuyCommand;
use crate::npc_shop_ui::{ordinary_gold_surface, action_retires_preentry, NpcShopUiContext,
    NpcShopUiState, NpcShopUiAction, NpcShopUiStamp, NpcShopUiView, NpcShopUiEffect, NPC_SHOP_VISIBLE_ROWS};
use crate::crystal_ui::shop_paint::{paint_npc_gold_shop, ShopPaintOptions, ShopPaintLabels,
    ShopPaintPanel, ShopPaintFeedback, ShopPaintControl, ShopPaintGoodCell, SHOP_REQUIRED_SKINS};
use crate::crystal_ui::item_image::layout_original_item_images;
use crate::portable_quest_ui::{QuestUiFont, QuestUiTargetCamera};
use crate::pending_operations::PendingLifecycleSet;

pub const SAFE: u64 = 9_007_199_254_740_991;
pub const SURFACE_WIDTH: f32 = 584.0;
pub const SURFACE_HEIGHT: f32 = 334.0;
pub fn required_skins() -> Vec<&'static str> { SHOP_REQUIRED_SKINS.to_vec() }

/// Full-u64 clocks are decimal strings at this ABI boundary, never JS numbers.
pub mod decimal_u64 {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn parse(value: &str) -> Option<u64> {
        if value.is_empty() || value.len() > 20 || value.starts_with('0')
            || !value.bytes().all(|b| b.is_ascii_digit()) { return None; }
        value.parse::<u64>().ok().filter(|v| *v != 0)
    }
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        if *value == 0 { return Err(serde::ser::Error::custom("zero shop clock")); }
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        parse(&text).ok_or_else(|| serde::de::Error::custom("invalid decimal shop clock"))
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct NpcShopSurfaceIdentity {
    pub run_generation:u64, pub connection_generation:u64,
    pub session_generation:u64, pub owner_revision:u64, pub player_object_id:u32,
}
impl NpcShopSurfaceIdentity {
    pub fn valid(self) -> bool {
        [self.run_generation,self.connection_generation,self.session_generation]
            .into_iter().all(|v| (1..=SAFE).contains(&v))
            && self.owner_revision <= SAFE && self.player_object_id != 0
    }
    pub fn can_follow(self, old:Self) -> bool {
        self.valid() && (self.run_generation > old.run_generation
            || self.run_generation == old.run_generation && (self.connection_generation > old.connection_generation
                || self.connection_generation == old.connection_generation && (self.session_generation > old.session_generation
                    || self.session_generation == old.session_generation && self.player_object_id == old.player_object_id
                        && self.owner_revision >= old.owner_revision)))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct NpcShopSurfacePresentation {
    pub logical_width:f32, pub logical_height:f32, pub stage_css_scale:f32, pub touch:bool,
}
impl NpcShopSurfacePresentation {
    pub fn scale(self) -> f32 { if self.touch { 1.28 / self.stage_css_scale } else { 1.0 } }
    pub fn valid(self) -> bool {
        [self.logical_width,self.logical_height,self.stage_css_scale].into_iter().all(f32::is_finite)
            && self.logical_width > 0.0 && self.logical_width <= 16384.0 && self.logical_width.fract() == 0.0
            && self.logical_height > 0.0 && self.logical_height <= 16384.0 && self.logical_height.fract() == 0.0
            && (0.05..=16.0).contains(&self.stage_css_scale)
            && SURFACE_WIDTH * self.scale() + 16.0 <= self.logical_width
            && SURFACE_HEIGHT * self.scale() + 16.0 <= self.logical_height
    }
    pub fn matches_window(self, width:f32, height:f32) -> bool {
        self.valid() && (width-self.logical_width).abs() <= 1.0 && (height-self.logical_height).abs() <= 1.0
    }
}
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct NpcShopSurfaceContext {
    pub identity:NpcShopSurfaceIdentity, pub revision:u64, pub model_revision:u64,
    pub presentation_revision:u64, pub service_revision:u64, pub catalog_revision:u64,
    pub core_authority_revision:Option<u64>, pub open:bool, pub input_enabled:bool,
    pub show_buy:bool, pub ready:bool, pub presentation:Option<NpcShopSurfacePresentation>,
    pub language:String, pub feedback:NpcGoldBuyFeedback,
}
impl NpcShopSurfaceContext {
    pub fn valid(&self) -> bool {
        self.identity.valid() && [self.revision,self.model_revision,self.presentation_revision,
            self.service_revision,self.catalog_revision].into_iter().all(|v| (1..=SAFE).contains(&v))
            && self.core_authority_revision.is_none_or(|v| v != 0)
            && (!self.input_enabled || self.open && self.show_buy && self.core_authority_revision.is_some())
            && self.presentation.is_none_or(NpcShopSurfacePresentation::valid)
            && self.language.len() <= 32 && self.feedback.previous_unknown <= 32
    }
    fn common(&self) -> NpcShopUiContext {
        let valid = self.valid();
        NpcShopUiContext { source_revision:self.core_authority_revision,
            open:self.open && valid, input_enabled:self.input_enabled && valid,
            show_buy:self.show_buy }
    }
}
#[derive(Resource, Debug, Clone, Default)]
pub struct NpcShopSurfaceReadModel {
    pub shop:ShopModel, pub inventory:InventoryModel, pub player:PlayerStats,
}
impl NpcShopSurfaceReadModel {
    fn key(&self) -> Option<String> {
        let mut shop = self.shop.clone(); shop.selected_id = None;
        shop.selected_bag_slot_for_sell = None; shop.selected_bag_slot_for_repair = None;
        let key = serde_json::to_string(&(shop, &self.inventory, &self.player)).ok()?;
        (key.len() <= 1_048_576).then_some(key)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct NpcShopSurfaceProof {
    #[serde(flatten)] pub identity:NpcShopSurfaceIdentity,
    pub revision:u64, pub model_revision:u64, pub presentation_revision:u64,
    pub service_revision:u64, pub catalog_revision:u64,
    #[serde(with="decimal_u64")] pub core_authority_revision:u64,
    #[serde(with="decimal_u64")] pub control_revision:u64,
}
impl NpcShopSurfaceProof {
    pub fn valid(self) -> bool {
        self.identity.valid() && [self.revision,self.model_revision,self.presentation_revision,
            self.service_revision,self.catalog_revision].into_iter().all(|v| (1..=SAFE).contains(&v))
            && self.core_authority_revision != 0 && self.control_revision != 0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub enum NpcShopSurfacePointerOrigin { Shop, World }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub enum NpcShopSurfacePointerPhase { Down, Move, Up, Cancel, Blur }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct NpcShopSurfaceGesture {
    pub pointer_id:u64, pub down_sequence:u64, pub sequence:u64,
    pub origin:NpcShopSurfacePointerOrigin, pub button:u8,
}
impl NpcShopSurfaceGesture {
    pub fn valid(self) -> bool {
        self.pointer_id <= SAFE && (1..=SAFE).contains(&self.down_sequence)
            && (self.down_sequence..=SAFE).contains(&self.sequence) && matches!(self.button,0|2)
    }
    pub fn matches_lease(self, other:Self) -> bool {
        self.pointer_id == other.pointer_id && self.down_sequence == other.down_sequence
            && self.origin == other.origin && self.button == other.button
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct NpcShopSurfacePointerEdge {
    #[serde(flatten)] pub proof:NpcShopSurfaceProof,
    #[serde(flatten)] pub gesture:NpcShopSurfaceGesture,
    pub phase:NpcShopSurfacePointerPhase, pub x:f32, pub y:f32,
}
impl NpcShopSurfacePointerEdge {
    pub fn valid(self) -> bool {
        self.proof.valid() && self.gesture.valid()
            && (self.phase != NpcShopSurfacePointerPhase::Down || self.gesture.down_sequence == self.gesture.sequence)
            && [self.x,self.y].into_iter().all(|v| v.is_finite() && v.abs() <= 16384.0)
    }
    pub fn matches_lease(self, other:Self) -> bool {
        self.proof == other.proof && self.gesture.matches_lease(other.gesture)
    }
}
#[derive(Resource, Default)]
pub struct NpcShopSurfacePointerEdges(pub Vec<NpcShopSurfacePointerEdge>);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag="type",rename_all="camelCase",deny_unknown_fields)]
pub enum NpcShopSurfaceAction {
    Select { #[serde(rename="uniqueId")] unique_id:u64 }, QuantityInc, QuantityDec,
    PageUp, PageDown, Buy, Close, HandoffSell,
}
impl From<NpcShopUiAction> for NpcShopSurfaceAction {
    fn from(action:NpcShopUiAction) -> Self { match action {
        NpcShopUiAction::Select(unique_id) => Self::Select {unique_id},
        NpcShopUiAction::QuantityInc => Self::QuantityInc, NpcShopUiAction::QuantityDec => Self::QuantityDec,
        NpcShopUiAction::PageUp => Self::PageUp, NpcShopUiAction::PageDown => Self::PageDown,
        NpcShopUiAction::Buy => Self::Buy, NpcShopUiAction::Close => Self::Close,
        NpcShopUiAction::HandoffSell => Self::HandoffSell,
    }}
}
impl From<NpcShopSurfaceAction> for NpcShopUiAction {
    fn from(action:NpcShopSurfaceAction) -> Self { match action {
        NpcShopSurfaceAction::Select {unique_id} => Self::Select(unique_id),
        NpcShopSurfaceAction::QuantityInc => Self::QuantityInc, NpcShopSurfaceAction::QuantityDec => Self::QuantityDec,
        NpcShopSurfaceAction::PageUp => Self::PageUp, NpcShopSurfaceAction::PageDown => Self::PageDown,
        NpcShopSurfaceAction::Buy => Self::Buy, NpcShopSurfaceAction::Close => Self::Close,
        NpcShopSurfaceAction::HandoffSell => Self::HandoffSell,
    }}
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NpcShopSurfaceIntent {
    pub proof:NpcShopSurfaceProof, pub gesture:NpcShopSurfaceGesture, pub intent_sequence:u64,
    pub action:NpcShopSurfaceAction, pub selected_id:Option<u64>, pub quantity:u16, pub start_index:usize,
    pub preentry_withdraw:bool, pub command:Option<NpcGoldBuyCommand>,
}
#[derive(Resource, Default)]
pub struct NpcShopSurfaceIntentQueue { next:u64, intents:Vec<NpcShopSurfaceIntent> }
impl NpcShopSurfaceIntentQueue {
    pub fn drain(&mut self) -> Vec<NpcShopSurfaceIntent> { std::mem::take(&mut self.intents) }
    pub fn clear(&mut self) { self.intents.clear(); }
    fn next(&mut self) -> Option<u64> {
        let next = self.next.checked_add(1).filter(|v| *v <= SAFE)?;
        if self.intents.len() >= 32 { return None; }
        self.next = next; Some(next)
    }
}
#[derive(Debug,Clone,Copy)]
struct PointerLease { edge:NpcShopSurfacePointerEdge, start:Option<NpcShopUiAction> }
#[derive(Debug,Clone)]
struct Pending { intent:NpcShopSurfaceIntent, read_key:String }
#[derive(Resource, Default)]
pub struct NpcShopSurfaceState {
    common:NpcShopUiState, pending:Option<Pending>, lease:Option<PointerLease>,
    last_pointer_sequence:u64, last_context:Option<NpcShopSurfaceContext>, read_key:Option<String>,
    pub error:Option<&'static str>,
}
impl NpcShopSurfaceState {
    pub fn cancel_pointer(&mut self) { self.lease = None; self.pending = None; }
    pub fn invalidate(&mut self) {
        self.cancel_pointer(); self.common.selected_id = None; self.common.quantity = 1; self.common.start_index = 0;
        self.common.invalidate_presentation(); self.read_key = None;
    }
    pub fn reconcile(&mut self,c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel) -> NpcShopUiEffect {
        let identity_changed = self.last_context.as_ref().is_none_or(|old| old.identity != c.identity);
        let service_changed = self.last_context.as_ref().is_some_and(|old|
            old.service_revision != c.service_revision || old.catalog_revision != c.catalog_revision);
        let presentation_changed = self.last_context.as_ref().is_some_and(|old|
            old.presentation != c.presentation || old.presentation_revision != c.presentation_revision
                || old.language != c.language || old.feedback != c.feedback);
        if identity_changed || service_changed {
            self.common.selected_id = None; self.common.quantity = 1; self.common.start_index = 0;
            self.common.invalidate_presentation();
        } else if presentation_changed { self.common.invalidate_presentation(); }
        let mut effect = self.common.reconcile(c.common(), &r.shop, &r.inventory);
        effect.changed |= identity_changed || service_changed || presentation_changed;
        effect.preentry_withdraw |= effect.changed;
        let key = r.key();
        let old_proof = self.last_context.as_ref().map(|old|
            (old.identity,old.revision,old.model_revision,old.presentation_revision,old.service_revision,
                old.catalog_revision,old.core_authority_revision,old.open,old.input_enabled,old.show_buy));
        let next_proof = (c.identity,c.revision,c.model_revision,c.presentation_revision,c.service_revision,
            c.catalog_revision,c.core_authority_revision,c.open,c.input_enabled,c.show_buy);
        if old_proof != Some(next_proof) || self.read_key != key || effect.changed {
            self.cancel_pointer();
        }
        // Each identity owns a pointer sequence; intent sequences never reset.
        // Old identity terminals are rejected before touching the new watermark.
        if identity_changed { self.last_pointer_sequence = 0; }
        self.last_context = Some(c.clone()); self.read_key = key;
        effect
    }
    pub fn view(&self,c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel) -> NpcShopUiView {
        let mut view = self.common.view(c.common(), &r.shop, &r.inventory, c.feedback);
        // Runtime publishes ready independently of the snapshot. Every other
        // context field must have been reconciled before exposing a current proof.
        let context_current = self.last_context.as_ref().is_some_and(|old|
            old.identity == c.identity && old.revision == c.revision && old.model_revision == c.model_revision
                && old.presentation_revision == c.presentation_revision && old.service_revision == c.service_revision
                && old.catalog_revision == c.catalog_revision && old.core_authority_revision == c.core_authority_revision
                && old.open == c.open && old.input_enabled == c.input_enabled && old.show_buy == c.show_buy
                && old.presentation == c.presentation && old.language == c.language && old.feedback == c.feedback);
        if !c.valid() || !context_current || self.read_key.is_none() || self.read_key != r.key() {
            view.stamp = None; view.can_buy = false; view.can_select = false; view.can_inc = false;
            view.can_dec = false; view.can_page_up = false; view.can_page_down = false;
        }
        view
    }
    pub fn proof(&self,c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel) -> Option<NpcShopSurfaceProof> {
        let stamp = self.view(c,r).stamp?;
        let proof = NpcShopSurfaceProof { identity:c.identity, revision:c.revision,
            model_revision:c.model_revision, presentation_revision:c.presentation_revision,
            service_revision:c.service_revision, catalog_revision:c.catalog_revision,
            core_authority_revision:stamp.source_revision, control_revision:stamp.presentation_revision };
        proof.valid().then_some(proof)
    }
    fn preview(&self,action:NpcShopUiAction,c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel) -> Option<NpcShopUiEffect> {
        let mut preview = self.common.clone();
        let effect = preview.apply(action,c.common(),&r.shop,&r.inventory,c.feedback);
        (effect.preentry_withdraw || effect.buy.is_some()).then_some(effect)
    }
    pub fn request_action(&mut self,action:NpcShopUiAction,gesture:NpcShopSurfaceGesture,
        c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel,q:&mut NpcShopSurfaceIntentQueue) -> bool {
        if self.pending.is_some() || self.error.is_some() || !c.ready || !c.open || !c.input_enabled
            || !gesture.valid() || gesture.origin != NpcShopSurfacePointerOrigin::Shop || gesture.button != 0 { return false; }
        let Some(proof) = self.proof(c,r) else { return false; };
        let Some(read_key) = r.key() else { return false; };
        let Some(effect) = self.preview(action,c,r) else { return false; };
        let Some(sequence) = q.next() else { self.error = Some("shop intent sequence exhausted or queue full"); return false; };
        let view = self.view(c,r);
        let intent = NpcShopSurfaceIntent { proof,gesture,intent_sequence:sequence,action:action.into(),
            selected_id:view.selected_id,quantity:view.quantity,start_index:view.start_index,
            preentry_withdraw:action_retires_preentry(action),command:effect.buy };
        self.pending = Some(Pending {intent:intent.clone(),read_key}); q.intents.push(intent); true
    }
    pub fn allows(&self,intent:&NpcShopSurfaceIntent,c:&NpcShopSurfaceContext,r:&NpcShopSurfaceReadModel) -> bool {
        let Some(pending) = &self.pending else { return false; };
        c.ready && c.input_enabled && c.open && self.error.is_none()
            && pending.intent == *intent && r.key().as_ref() == Some(&pending.read_key)
            && self.proof(c,r) == Some(intent.proof)
            && self.preview(intent.action.into(),c,r).is_some_and(|effect| effect.buy == intent.command)
    }
    pub fn apply_accepted(&mut self,intent:&NpcShopSurfaceIntent,c:&NpcShopSurfaceContext,
        r:&NpcShopSurfaceReadModel) -> Option<NpcShopUiEffect> {
        if !self.allows(intent,c,r) { return None; }
        self.pending = None;
        Some(self.common.apply(intent.action.into(),c.common(),&r.shop,&r.inventory,c.feedback))
    }
    pub fn finish_rejected(&mut self,intent:&NpcShopSurfaceIntent) -> bool {
        if !self.pending.as_ref().is_some_and(|pending| pending.intent == *intent) { return false; }
        self.pending = None; true
    }
}

#[derive(Component)]
pub struct NpcShopSurfaceRoot;
#[derive(Component)]
pub struct NpcShopSurfaceViewport;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcShopSurfaceControl(pub NpcShopSurfaceProof);
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcShopSurfaceTreeStamp {
    pub proof:NpcShopSurfaceProof, pub selected_id:Option<u64>, pub quantity:u16, pub start_index:usize,
}
impl NpcShopSurfaceTreeStamp {
    pub fn current(self,c:&NpcShopSurfaceContext,s:&NpcShopSurfaceState,r:&NpcShopSurfaceReadModel) -> bool {
        let view = s.view(c,r);
        s.proof(c,r) == Some(self.proof) && self.selected_id == view.selected_id
            && self.quantity == view.quantity && self.start_index == view.start_index
    }
}
pub fn logical_rect(node:&ComputedNode,transform:&bevy::ui::UiGlobalTransform) -> Option<bevy::math::Rect> {
    let scale = node.inverse_scale_factor;
    let center = transform.affine().translation * scale; let size = node.size() * scale;
    if !center.is_finite() || !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 { return None; }
    Some(bevy::math::Rect::from_center_size(center,size))
}
fn descendant(mut entity:Entity,root:Entity,parents:&Query<&ChildOf>) -> bool {
    while let Ok(parent) = parents.get(entity) { entity = parent.parent(); if entity == root { return true; } }
    false
}
fn displayed(mut entity:Entity,parents:&Query<&ChildOf>,nodes:&Query<&Node>) -> bool {
    loop {
        if nodes.get(entity).is_ok_and(|node| node.display == Display::None) { return false; }
        let Ok(parent) = parents.get(entity) else { return true; }; entity = parent.parent();
    }
}
fn visible_rect(entity:Entity,node:&ComputedNode,transform:&bevy::ui::UiGlobalTransform,
    clip:Option<&bevy::ui::CalculatedClip>) -> Option<bevy::math::Rect> {
    let mut rect = logical_rect(node,transform)?;
    if let Some(clip) = clip {
        // CalculatedClip is in physical layout coordinates like UiGlobalTransform.
        let scale = node.inverse_scale_factor;
        if !clip.clip.min.is_finite() || !clip.clip.max.is_finite() { return None; }
        rect = rect.intersect(bevy::math::Rect::from_corners(clip.clip.min*scale,clip.clip.max*scale));
    }
    let _ = entity;
    (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect)
}
fn rect_same(a:bevy::math::Rect,b:bevy::math::Rect) -> bool {
    (a.min-b.min).abs().max_element() <= 0.1 && (a.max-b.max).abs().max_element() <= 0.1
}

pub struct Mir2PortableNpcShopUiPlugin;
impl Plugin for Mir2PortableNpcShopUiPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<NpcShopSurfaceContext>().init_resource::<NpcShopSurfaceReadModel>()
            .init_resource::<NpcShopSurfaceState>().init_resource::<NpcShopSurfacePointerEdges>()
            .init_resource::<NpcShopSurfaceIntentQueue>()
            .add_systems(Startup,spawn_root)
            .add_systems(Update,(apply_context,process_pointer,render_shop,layout_original_item_images)
                .chain().after(PendingLifecycleSet::Ingest));
    }
}
fn spawn_root(mut commands:Commands,camera:Option<Res<QuestUiTargetCamera>>) {
    let Some(camera) = camera else { return; };
    // This surface scales the original art and notice together. Bevy 0.19
    // LayoutConfig propagates to descendants; retaining fractional coordinates
    // avoids a false readiness failure at touch/DPR scales without widening
    // any clip or design-geometry tolerance.
    commands.spawn((NpcShopSurfaceRoot,UiTargetCamera(camera.0),bevy::ui::LayoutConfig {use_rounding:false},Node {
        position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),
        display:Display::None,..default()
    },FocusPolicy::Pass,GlobalZIndex(998)));
}
fn apply_context(c:Res<NpcShopSurfaceContext>,r:Res<NpcShopSurfaceReadModel>,
    mut s:ResMut<NpcShopSurfaceState>,mut q:ResMut<NpcShopSurfaceIntentQueue>) {
    s.reconcile(&c,&r);
    if s.pending.is_none() { q.clear(); }
}
#[allow(clippy::type_complexity)]
fn hit(point:Vec2,root:Entity,proof:NpcShopSurfaceProof,
    actions:&Query<(Entity,&NpcShopUiAction,&ShopPaintControl,&NpcShopUiStamp,&NpcShopSurfaceControl,
        &ComputedNode,&bevy::ui::UiGlobalTransform,&InheritedVisibility,Option<&bevy::ui::CalculatedClip>,Option<&Button>)>,
    parents:&Query<&ChildOf>) -> Option<NpcShopUiAction> {
    actions.iter().filter(|(entity,action,control,stamp,host,node,transform,visible,clip,button)| {
        descendant(*entity,root,parents) && visible.get() && control.enabled && control.action == **action
            && button.is_some() && host.0 == proof && stamp.source_revision == proof.core_authority_revision
            && stamp.presentation_revision == proof.control_revision
            && visible_rect(*entity,node,transform,*clip).is_some_and(|rect| rect.contains(point))
    }).min_by_key(|(entity,..)| entity.to_bits()).map(|(_,action,..)| *action)
}
#[allow(clippy::type_complexity)]
fn process_pointer(c:Res<NpcShopSurfaceContext>,r:Res<NpcShopSurfaceReadModel>,mut s:ResMut<NpcShopSurfaceState>,
    mut edges:ResMut<NpcShopSurfacePointerEdges>,mut q:ResMut<NpcShopSurfaceIntentQueue>,
    roots:Query<(Entity,&NpcShopSurfaceTreeStamp),With<NpcShopSurfaceRoot>>,
    actions:Query<(Entity,&NpcShopUiAction,&ShopPaintControl,&NpcShopUiStamp,&NpcShopSurfaceControl,
        &ComputedNode,&bevy::ui::UiGlobalTransform,&InheritedVisibility,Option<&bevy::ui::CalculatedClip>,Option<&Button>)>,
    parents:Query<&ChildOf>) {
    for edge in std::mem::take(&mut edges.0) {
        if !edge.valid() { continue; }
        let current = s.proof(&c,&r) == Some(edge.proof);
        if !current {
            if matches!(edge.phase,NpcShopSurfacePointerPhase::Up|NpcShopSurfacePointerPhase::Cancel|NpcShopSurfacePointerPhase::Blur)
                && s.lease.is_some_and(|lease| edge.matches_lease(lease.edge)) { s.lease = None; }
            continue;
        }
        if edge.gesture.sequence <= s.last_pointer_sequence { continue; }
        if edge.phase != NpcShopSurfacePointerPhase::Down
            && !s.lease.is_some_and(|lease| edge.matches_lease(lease.edge)) { continue; }
        if edge.phase == NpcShopSurfacePointerPhase::Down && s.lease.is_some() { continue; }
        s.last_pointer_sequence = edge.gesture.sequence;
        if edge.phase == NpcShopSurfacePointerPhase::Cancel || edge.phase == NpcShopSurfacePointerPhase::Blur {
            if s.lease.is_some_and(|lease| edge.matches_lease(lease.edge)) { s.lease = None; }
            // The terminal of another down gesture cannot erase awaiting work.
            if s.pending.as_ref().is_some_and(|pending| pending.intent.proof == edge.proof
                && pending.intent.gesture.matches_lease(edge.gesture)) { s.pending = None; q.clear(); }
            continue;
        }
        let root = roots.single().ok().filter(|(_,stamp)| stamp.current(&c,&s,&r)).map(|(entity,_)| entity);
        if !c.ready || !c.open || !c.input_enabled || s.error.is_some() || root.is_none() {
            if s.lease.is_some_and(|lease| edge.matches_lease(lease.edge)) { s.lease = None; }
            continue;
        }
        let root = root.unwrap(); let point = Vec2::new(edge.x,edge.y);
        match edge.phase {
            NpcShopSurfacePointerPhase::Down => {
                s.pending = None; q.clear();
                let start = (edge.gesture.origin == NpcShopSurfacePointerOrigin::Shop && edge.gesture.button == 0)
                    .then(|| hit(point,root,edge.proof,&actions,&parents)).flatten();
                s.lease = Some(PointerLease {edge,start});
            }
            NpcShopSurfacePointerPhase::Move => {},
            NpcShopSurfacePointerPhase::Up => {
                let Some(lease) = s.lease.filter(|lease| edge.matches_lease(lease.edge)) else { continue; };
                s.lease = None;
                if edge.gesture.origin != NpcShopSurfacePointerOrigin::Shop || edge.gesture.button != 0 { continue; }
                let Some(start) = lease.start else { continue; };
                if hit(point,root,edge.proof,&actions,&parents) == Some(start) {
                    s.request_action(start,edge.gesture,&c,&r,&mut q);
                }
            }
            NpcShopSurfacePointerPhase::Cancel|NpcShopSurfacePointerPhase::Blur => {},
        }
    }
}
fn labels(language:&str) -> ShopPaintLabels {
    match language {
        "zh-CN" => ShopPaintLabels {
            no_goods:"没有商品".into(),gold:"金币".into(),total:"总价".into(),
            waiting:"购买等待发送".into(),entered:"购买发送中，结果尚未确认".into(),
            flushed:"购买已发送，结果尚未确认".into(),unknown:"购买结果未知，不会自动重试".into(),
            definitely_unsent:"购买未发送，可手动重新购买".into(),blocked:"当前不能购买此商品".into(),
        },
        "ja" => ShopPaintLabels {
            no_goods:"商品なし".into(),gold:"ゴールド".into(),total:"合計".into(),
            waiting:"購入の送信待ち".into(),entered:"購入を送信中、結果未確認".into(),
            flushed:"購入送信済み、結果未確認".into(),unknown:"購入結果不明、自動再試行なし".into(),
            definitely_unsent:"購入は未送信、手動で再試行できます".into(),blocked:"この商品は現在購入できません".into(),
        },
        _ => ShopPaintLabels::default(),
    }
}
#[allow(clippy::type_complexity)]
fn render_shop(mut commands:Commands,c:Res<NpcShopSurfaceContext>,r:Res<NpcShopSurfaceReadModel>,s:Res<NpcShopSurfaceState>,
    roots:Query<Entity,With<NpcShopSurfaceRoot>>,mut nodes:Query<&mut Node,With<NpcShopSurfaceRoot>>,
    font:Option<Res<QuestUiFont>>,server:Option<Res<AssetServer>>,
    mut last:Local<Option<(NpcShopSurfaceTreeStamp,bool,NpcGoldBuyFeedback,String,NpcShopSurfacePresentation,Handle<Font>)>>) {
    let Ok(root) = roots.single() else { return; }; let Ok(mut node) = nodes.get_mut(root) else { return; };
    let view = s.view(&c,&r);
    let proof = s.proof(&c,&r);
    let visible = c.valid() && c.open && c.show_buy && ordinary_gold_surface(&r.shop,true)
        && c.presentation.is_some_and(NpcShopSurfacePresentation::valid) && proof.is_some() && s.error.is_none();
    node.display = if visible {Display::Flex} else {Display::None};
    if !visible {
        *last = None; commands.entity(root).remove::<NpcShopSurfaceTreeStamp>().despawn_children(); return;
    }
    let (Some(font),Some(server),Some(p),Some(proof)) = (font,server,c.presentation,proof) else {
        node.display = Display::None; *last = None;
        commands.entity(root).remove::<NpcShopSurfaceTreeStamp>().despawn_children(); return;
    };
    let stamp = NpcShopSurfaceTreeStamp {proof,selected_id:view.selected_id,quantity:view.quantity,start_index:view.start_index};
    let key = (stamp,c.input_enabled,c.feedback,c.language.clone(),p,font.0.clone());
    if last.as_ref() == Some(&key) && !r.is_changed() { return; } *last = Some(key);
    let scale = p.scale(); let width = SURFACE_WIDTH * scale; let height = SURFACE_HEIGHT * scale;
    commands.entity(root).insert(stamp).despawn_children();
    commands.entity(root).with_children(|parent| {
        parent.spawn((NpcShopSurfaceViewport,Node {position_type:PositionType::Absolute,
            left:Val::Px((p.logical_width-width)*0.5),top:Val::Px((p.logical_height-height)*0.5),
            width:Val::Px(width),height:Val::Px(height),overflow:Overflow::clip(),..default()},FocusPolicy::Block))
            .with_children(|viewport| {
                let options = ShopPaintOptions {font:Some(TextFont {font:bevy::text::FontSource::Handle(font.0.clone()),
                    font_size:bevy::text::FontSize::Px(10.0),..default()}),scale,labels:labels(&c.language)};
                paint_npc_gold_shop(viewport,Some(&server),&r.shop,&r.player,&view,&options,
                    |_| NpcShopSurfaceControl(proof));
            });
    });
}

#[derive(Default)]
pub struct NpcShopSurfaceTreeObservation {
    pub complete:bool, pub input_regions:Vec<bevy::math::Rect>,
}
#[derive(bevy::ecs::system::SystemParam)]
pub struct NpcShopSurfaceTreeObserver<'w,'s> {
    roots:Query<'w,'s,(Entity,&'static NpcShopSurfaceTreeStamp,&'static ComputedNode,
        &'static bevy::ui::UiGlobalTransform,&'static InheritedVisibility,&'static bevy::ui::LayoutConfig),With<NpcShopSurfaceRoot>>,
    viewports:Query<'w,'s,(Entity,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform,
        &'static InheritedVisibility,Option<&'static bevy::ui::CalculatedClip>),With<NpcShopSurfaceViewport>>,
    panels:Query<'w,'s,(Entity,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform,
        &'static InheritedVisibility,Option<&'static bevy::ui::CalculatedClip>),With<ShopPaintPanel>>,
    notices:Query<'w,'s,(Entity,&'static ComputedNode,&'static bevy::ui::UiGlobalTransform,
        &'static InheritedVisibility,Option<&'static bevy::ui::CalculatedClip>),With<ShopPaintFeedback>>,
    controls:Query<'w,'s,(Entity,&'static ShopPaintControl,&'static NpcShopUiStamp,
        Option<&'static NpcShopUiAction>,Option<&'static NpcShopSurfaceControl>,Option<&'static Button>,
        &'static ComputedNode,&'static bevy::ui::UiGlobalTransform,&'static InheritedVisibility,
        Option<&'static bevy::ui::CalculatedClip>)>,
    cells:Query<'w,'s,(Entity,&'static ShopPaintGoodCell)>,
    texts:Query<'w,'s,(Entity,&'static Text,Option<&'static bevy::text::TextLayoutInfo>,
        &'static ComputedNode,&'static bevy::ui::UiGlobalTransform,&'static InheritedVisibility,
        Option<&'static bevy::ui::CalculatedClip>)>,
    parents:Query<'w,'s,&'static ChildOf>, nodes:Query<'w,'s,&'static Node>,
}
impl NpcShopSurfaceTreeObserver<'_,'_> {
    pub fn observe(&self,c:&NpcShopSurfaceContext,s:&NpcShopSurfaceState,r:&NpcShopSurfaceReadModel) -> NpcShopSurfaceTreeObservation {
        let empty = || NpcShopSurfaceTreeObservation::default();
        let Some(p) = c.presentation.filter(|p| p.valid()) else { return empty(); };
        if !c.valid() || !c.open || !c.show_buy || !ordinary_gold_surface(&r.shop,true) || s.error.is_some() { return empty(); }
        let Ok((root,stamp,node,transform,visible,layout_config)) = self.roots.single() else { return empty(); };
        if layout_config.use_rounding || !visible.get() || !displayed(root,&self.parents,&self.nodes) || !stamp.current(c,s,r)
            || logical_rect(node,transform).is_none() { return empty(); }
        let stage = bevy::math::Rect::from_corners(Vec2::ZERO,Vec2::new(p.logical_width,p.logical_height));
        if !logical_rect(node,transform).is_some_and(|rect| rect_same(rect,stage)) { return empty(); }
        let in_stage = |rect:bevy::math::Rect| rect.min.x >= -0.1 && rect.min.y >= -0.1
            && rect.max.x <= stage.max.x+0.1 && rect.max.y <= stage.max.y+0.1;
        let measured = |entity,node,transform,visible:&InheritedVisibility,clip| {
            if !visible.get() || !displayed(entity,&self.parents,&self.nodes) { return None; }
            let full = logical_rect(node,transform)?;
            let clipped = visible_rect(entity,node,transform,clip)?;
            (rect_same(full,clipped) && in_stage(full)).then_some(full)
        };
        let mut views = self.viewports.iter().filter(|(entity,..)| descendant(*entity,root,&self.parents));
        let Some((viewport,vnode,vtransform,vvisible,vclip)) = views.next() else { return empty(); };
        if views.next().is_some() { return empty(); }
        let Some(viewport_rect) = measured(viewport,vnode,vtransform,vvisible,vclip) else { return empty(); };
        let scale = p.scale();
        if (viewport_rect.width()-SURFACE_WIDTH*scale).abs() > 0.1
            || (viewport_rect.height()-SURFACE_HEIGHT*scale).abs() > 0.1 { return empty(); }
        let mut panels = self.panels.iter().filter(|(entity,..)| descendant(*entity,viewport,&self.parents));
        let Some((panel,pnode,ptransform,pvisible,pclip)) = panels.next() else { return empty(); };
        if panels.next().is_some() { return empty(); }
        let Some(panel_rect) = measured(panel,pnode,ptransform,pvisible,pclip) else { return empty(); };
        let expected_panel = bevy::math::Rect::from_corners(viewport_rect.min,viewport_rect.min+Vec2::new(244.0,334.0)*scale);
        if !rect_same(panel_rect,expected_panel) { return empty(); }
        let mut notices = self.notices.iter().filter(|(entity,..)| descendant(*entity,panel,&self.parents));
        let Some((notice,nnode,ntransform,nvisible,nclip)) = notices.next() else { return empty(); };
        if notices.next().is_some() { return empty(); }
        let Some(notice_rect) = measured(notice,nnode,ntransform,nvisible,nclip) else { return empty(); };
        let expected_notice = bevy::math::Rect::from_corners(viewport_rect.min+Vec2::new(254.0,34.0)*scale,
            viewport_rect.min+Vec2::new(584.0,114.0)*scale);
        if !rect_same(notice_rect,expected_notice) { return empty(); }
        let view = s.view(c,r);
        let Some(common_stamp) = view.stamp else { return empty(); };
        let mut expected:Vec<(NpcShopUiAction,bool)> = vec![
            (NpcShopUiAction::Close,view.can_select),(NpcShopUiAction::PageUp,view.can_page_up),
            (NpcShopUiAction::PageDown,view.can_page_down),(NpcShopUiAction::Buy,view.can_buy),
            (NpcShopUiAction::QuantityDec,view.can_dec),(NpcShopUiAction::QuantityInc,view.can_inc)];
        if view.show_sell { expected.push((NpcShopUiAction::HandoffSell,view.can_select)); }
        let goods:Vec<_> = r.shop.goods.iter().skip(view.start_index).take(NPC_SHOP_VISIBLE_ROWS).collect();
        expected.extend(goods.iter().map(|good| (NpcShopUiAction::Select(good.unique_id),view.can_select)));
        let mut seen = Vec::new();
        for (entity,control,control_stamp,action,host,button,node,transform,visible,clip) in &self.controls {
            if !descendant(entity,panel,&self.parents) { continue; }
            if *control_stamp != common_stamp || !expected.contains(&(control.action,control.enabled))
                || seen.contains(&control.action) { return empty(); }
            if control.enabled {
                if action.copied() != Some(control.action) || host.is_none_or(|host| host.0 != stamp.proof)
                    || button.is_none() { return empty(); }
            } else if action.is_some() || host.is_some() || button.is_some() { return empty(); }
            let Some(rect) = measured(entity,node,transform,visible,clip) else { return empty(); };
            let design = match control.action {
                NpcShopUiAction::Close => Some((217.0,3.0,24.0,21.0)),
                NpcShopUiAction::PageUp => Some((219.0,35.0,12.0,12.0)),
                NpcShopUiAction::PageDown => Some((219.0,284.0,12.0,12.0)),
                NpcShopUiAction::Buy => Some((77.0,304.0,80.0,25.0)),
                NpcShopUiAction::QuantityDec => Some((12.0,304.0,20.0,22.0)),
                NpcShopUiAction::QuantityInc => Some((58.0,304.0,18.0,22.0)),
                NpcShopUiAction::HandoffSell => Some((162.0,304.0,52.0,22.0)),
                NpcShopUiAction::Select(_) => None,
            };
            if let Some((x,y,width,height)) = design {
                let min = panel_rect.min+Vec2::new(x,y)*scale;
                if !rect_same(rect,bevy::math::Rect::from_corners(min,min+Vec2::new(width,height)*scale)) { return empty(); }
            }
            seen.push(control.action);
        }
        if seen.len() != expected.len() { return empty(); }
        let mut cell_ids = Vec::new();
        for (entity,cell) in &self.cells {
            if !descendant(entity,panel,&self.parents) { continue; }
            let Some(index) = goods.iter().position(|good| good.unique_id == cell.unique_id) else { return empty(); };
            if cell_ids.contains(&cell.unique_id) || cell.selected != (view.selected_id == Some(cell.unique_id)) { return empty(); }
            let Ok((_,control,_,_,_,_,node,transform,visible,clip)) = self.controls.get(entity) else { return empty(); };
            if control.action != NpcShopUiAction::Select(cell.unique_id) { return empty(); }
            let Some(rect) = measured(entity,node,transform,visible,clip) else { return empty(); };
            let top_left = panel_rect.min+Vec2::new(10.0,34.0+33.0*index as f32)*scale;
            if !rect_same(rect,bevy::math::Rect::from_corners(top_left,top_left+Vec2::new(205.0,32.0)*scale)) { return empty(); }
            cell_ids.push(cell.unique_id);
        }
        if cell_ids.len() != goods.len() { return empty(); }
        let mut text_count = 0;
        for (entity,text,layout,node,transform,visible,clip) in &self.texts {
            if !descendant(entity,panel,&self.parents) || text.0.trim().is_empty() { continue; }
            if !visible.get() || !displayed(entity,&self.parents,&self.nodes)
                || !layout.is_some_and(|layout| !layout.glyphs.is_empty())
                || visible_rect(entity,node,transform,clip).is_none() { return empty(); }
            text_count += 1;
        }
        if text_count < 2 { return empty(); }
        // Both regions come from actual computed nodes. The 584px notice extent
        // is mandatory even when its current text is empty.
        NpcShopSurfaceTreeObservation {complete:true,input_regions:vec![panel_rect,notice_rect]}
    }
}

#[cfg(test)]
#[path="portable_npc_shop_ui_tests.rs"]
mod tests;
