//! Hero Web host. Raw custody has one consumer; business operations stay in the
//! browser's existing ledger. No Native ACK settlement or request allocator.
use std::{cell::RefCell, collections::VecDeque};
use bevy::prelude::*;
use mir2_client_bevy::{
    hero_ingress::{HeroIngress, HeroScope, VerifiedHeroOwner},
    hero_model::HeroModel,
    portable_hero_ui::{
        HeroInputEdge, HeroInputQueue, HeroIntentQueue, HeroPresentation, HeroStamp,
        HeroUiContext, HeroUiIntent, HeroUiReadModel, HeroUiState, HeroWindows,
    },
    read_model::UiReadModel,
};
use serde::{Deserialize, Serialize};

const MAX_SAFE: u64 = 9_007_199_254_740_991;
const MAX_CONTROL_BYTES: usize = 8_192;
const MAX_INPUT_BYTES: usize = 8_192;
const MAX_INPUTS: usize = 64;
const MAX_HOST_CHECKPOINT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostCheckpoint {
    version: u8,
    scope: HeroScope,
    source_sequence: u64,
    ingress_checkpoint: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostInput {
    sink_generation: u64,
    control_revision: u64,
    web_lease_token: String,
    edge: HeroInputEdge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeroSource {
    pub scope: HeroScope,
    /// Last changed Hero delivery, separate from the accepted raw-frame cursor.
    pub frame_sequence: u64,
    pub hero_object_id: u32,
    pub hero_generation: u64,
    pub rust_model_revision: u64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeroControl {
    pub source: HeroSource,
    pub control_revision: u64,
    pub web_lease_token: String,
    pub hud_generation: u64,
    pub web_model_revision: u64,
    pub presentation_revision: u64,
    pub window_epochs: [u64; 3],
    pub windows: HeroWindows,
    pub presentation: Option<HeroPresentation>,
    pub in_game: bool,
    pub host_visible: bool,
    pub input_enabled: bool,
    pub pending: bool,
}
impl HeroControl {
    pub fn stamp(&self) -> HeroStamp {
        HeroStamp {
            scope: self.source.scope.clone(),
            hero_object_id: self.source.hero_object_id,
            hero_generation: self.source.hero_generation,
            hud_generation: self.hud_generation,
            model_revision: self.web_model_revision,
            presentation_revision: self.presentation_revision,
            window_epochs: self.window_epochs,
        }
    }
    fn valid(&self) -> bool {
        self.stamp().valid()
            && [self.control_revision, self.source.frame_sequence, self.source.rust_model_revision]
                .into_iter().all(|n| n > 0 && n <= MAX_SAFE)
            && !self.web_lease_token.is_empty() && self.web_lease_token.len() <= 512
            && !self.web_lease_token.contains('\0')
            && self.presentation.is_none_or(HeroPresentation::fits)
            && (!(self.in_game && self.host_visible) || self.presentation.is_some())
    }
}

/// Input/control bounds are additional to HeroIngress's own 32 MiB bound.
/// They do not claim to share that allocator or to cap all runtime resources.
pub struct HeroBridge {
    ingress: HeroIngress,
    source_sequence: u64,
    control: Option<HeroControl>,
    control_high: u64,
    input_high: u64,
    inputs: VecDeque<HeroInputEdge>,
    input_closed: bool,
    input_reset: bool,
    sink_present: bool,
    sink_generation: u64,
}
impl Default for HeroBridge {
    fn default() -> Self {
        Self { ingress: HeroIngress::default(), source_sequence: 0, control: None,
            control_high: 0, input_high: 0, inputs: VecDeque::new(),
            input_closed: false, input_reset: false, sink_present: false, sink_generation: 0 }
    }
}
impl HeroBridge {
    pub fn activate(&mut self, scope: HeroScope) -> bool {
        if self.ingress.is_current(&scope) {
            return !self.input_closed && !self.ingress.status().closed;
        }
        if self.ingress.activate(scope).is_err() { return false; }
        self.source_sequence = 0;
        self.control = None;
        self.control_high = 0;
        self.input_high = 0;
        self.inputs.clear();
        self.input_closed = false;
        self.input_reset = true;
        self.sink_present = false;
        self.sink_generation = self.sink_generation.saturating_add(1);
        true
    }
    pub fn withdraw(&mut self, scope: &HeroScope) -> bool {
        if !self.ingress.withdraw(scope) { return false; }
        self.control = None;
        self.inputs.clear();
        self.input_closed = true;
        self.input_reset = true;
        self.sink_present = false;
        self.sink_generation = self.sink_generation.saturating_add(1);
        true
    }
    pub fn push_raw(&mut self, scope: &HeroScope, sequence: u64, raw: &str, at: u64) -> bool {
        if self.input_closed { return false; }
        match self.ingress.receive_frame(scope, sequence, raw, at) {
            Ok(changed) => {
                if changed { self.source_sequence = sequence; }
                true
            }
            Err(_) => false,
        }
    }
    /// Display only: the browser has already applied this verified economic owner.
    /// No Core Applied, ACK or browser operation bookkeeping originates here.
    pub fn push_verified_owner(&mut self, scope: &HeroScope, sequence: u64, raw: &str,
        at: u64, expected: &VerifiedHeroOwner) -> bool {
        if self.input_closed { return false; }
        match self.ingress.receive_verified_owner_frame(scope, sequence, raw, at, expected) {
            Ok(changed) => {
                if changed { self.source_sequence = sequence; }
                true
            }
            Err(_) => false,
        }
    }
    pub fn source(&self) -> Option<HeroSource> {
        if !self.ingress.available() { return None; }
        let model = self.ingress.model();
        let info = model.info.as_ref()?;
        if model.snapshot_identity.is_none() || model.hero_generation == 0
            || model.revision == 0 || self.source_sequence == 0 { return None; }
        Some(HeroSource {
            scope: self.ingress.scope()?.clone(), frame_sequence: self.source_sequence,
            hero_object_id: info.object_id, hero_generation: model.hero_generation,
            rust_model_revision: model.revision,
        })
    }
    pub fn control_current(&self, control: &HeroControl) -> bool {
        !self.input_closed && !self.ingress.status().closed
            && self.source().as_ref() == Some(&control.source)
            && self.control.as_ref().is_some_and(|current|
                current.control_revision == control.control_revision
                    && current.web_lease_token == control.web_lease_token
                    && current.stamp() == control.stamp())
    }
    pub fn set_control(&mut self, scope: &HeroScope, raw: &str) -> bool {
        if raw.is_empty() || raw.len() > MAX_CONTROL_BYTES { return false; }
        let Ok(control) = serde_json::from_str::<HeroControl>(raw) else { return false; };
        if !control.valid() || control.source.scope != *scope
            || !self.ingress.is_current(scope) || self.input_closed
            || self.source().as_ref() != Some(&control.source)
            || control.control_revision <= self.control_high { return false; }
        let same_lease = self.control.as_ref().is_some_and(|old|
            old.stamp() == control.stamp() && old.web_lease_token == control.web_lease_token
                && old.windows == control.windows);
        if !same_lease || !control.in_game || !control.host_visible || !control.input_enabled {
            self.inputs.clear(); self.input_reset = true;
        }
        self.control_high = control.control_revision;
        self.control = Some(control);
        true
    }
    pub fn push_edge(&mut self, scope: &HeroScope, raw: &str) -> bool {
        if raw.is_empty() || raw.len() > MAX_INPUT_BYTES { return false; }
        let Ok(input) = serde_json::from_str::<HostInput>(raw) else { return false; };
        let edge = input.edge;
        let Some(control) = self.control.as_ref() else { return false; };
        if !self.sink_present || input.sink_generation != self.sink_generation
            || input.control_revision != control.control_revision
            || input.web_lease_token != control.web_lease_token
            || !edge.valid() || !self.control_current(control) || control.source.scope != *scope
            || edge.stamp != control.stamp() || edge.sequence <= self.input_high
            || !control.in_game || !control.host_visible || !control.input_enabled { return false; }
        if self.inputs.len() >= MAX_INPUTS {
            self.inputs.clear();
            self.input_closed = true;
            self.input_reset = true;
            return false;
        }
        self.input_high = edge.sequence;
        self.inputs.push_back(edge);
        true
    }
    pub fn checkpoint(&self, scope: &HeroScope) -> Option<String> {
        if !self.ingress.is_current(scope) || self.input_closed || !self.inputs.is_empty() {
            return None;
        }
        let checkpoint = HostCheckpoint { version: 1, scope: scope.clone(),
            source_sequence: self.source_sequence, ingress_checkpoint: self.ingress.checkpoint().ok()? };
        let raw = serde_json::to_string(&checkpoint).ok()?;
        (raw.len() <= MAX_HOST_CHECKPOINT_BYTES).then_some(raw)
    }
    pub fn restore(&mut self, scope: &HeroScope, raw: &str) -> bool {
        if !self.ingress.is_current(scope) || self.input_closed || raw.is_empty()
            || raw.len() > MAX_HOST_CHECKPOINT_BYTES { return false; }
        let Ok(checkpoint) = serde_json::from_str::<HostCheckpoint>(raw) else { return false; };
        if checkpoint.version != 1 || !checkpoint.scope.valid()
            || !checkpoint.scope.same_physical(scope)
            || checkpoint.scope.run_generation > scope.run_generation
            || serde_json::to_string(&checkpoint).ok().as_deref() != Some(raw) { return false; }
        // The ingress string stays intact. Inspect only its cursor/scope before
        // mutating the fresh receiver; the ingress validates the canonical body.
        let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&checkpoint.ingress_checkpoint)
            else { return false; };
        let Some(accepted) = metadata["frameSequence"].as_u64() else { return false; };
        if checkpoint.source_sequence > accepted || checkpoint.source_sequence > MAX_SAFE
            || serde_json::from_value::<HeroScope>(metadata["scope"].clone()).ok().as_ref()
                != Some(&checkpoint.scope) { return false; }
        if self.ingress.restore_checkpoint(scope, &checkpoint.ingress_checkpoint).is_err() { return false; }
        self.source_sequence = checkpoint.source_sequence;
        self.control = None;
        self.inputs.clear();
        self.input_reset = true;
        true
    }
    fn advance_sink(&mut self, scope: &HeroScope) -> Option<u64> {
        if !self.ingress.is_current(scope) || self.input_closed
            || self.sink_generation >= MAX_SAFE { return None; }
        self.sink_generation += 1;
        self.sink_present = true;
        self.inputs.clear();
        self.input_reset = true;
        Some(self.sink_generation)
    }
    fn clear_sink(&mut self, scope: &HeroScope, expected_generation: u64) -> bool {
        if !self.ingress.is_current(scope) || !self.sink_present
            || expected_generation != self.sink_generation { return false; }
        self.sink_generation += 1;
        self.sink_present = false;
        self.inputs.clear();
        self.input_reset = true;
        true
    }
    pub fn feedback_current(&self, control: &HeroControl, sink_generation: u64) -> bool {
        self.sink_present && sink_generation == self.sink_generation && self.control_current(control)
    }
}
thread_local! {
    static BRIDGE: RefCell<HeroBridge> = RefCell::new(HeroBridge::default());
}
#[derive(Resource, Default)]
struct AppliedHero {
    source: Option<HeroSource>,
    control: Option<HeroControl>,
    receipt_frames: Vec<u64>,
    #[cfg(target_arch = "wasm32")]
    assets: Vec<Handle<Image>>,
    #[cfg(target_arch = "wasm32")]
    asset_paths: Vec<String>,
}
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeroConsumeSet;

/// One ordered consumer, called by the actual Web schedule and bare-World tests.
/// Applying a receipt here never settles the browser's operation ledger.
fn apply_bridge_to_world(bridge: &mut HeroBridge, world: &mut World, now_ms: u64) {
    let mut receipt_frames = Vec::new();
    if let Ok(Some(batch)) = bridge.ingress.take_batch() {
        if bridge.ingress.batch_is_current(&batch) {
            for receipt in batch.receipts {
                receipt_frames.push(receipt.frame_sequence);
                world.insert_resource(receipt.model);
            }
            if let Some(latest) = batch.latest { world.insert_resource(latest.model); }
        }
    }
    // Reapply after an unscoped protocol reset, even if this source was already
    // drained in a prior frame. The permanent ingress lease is never reset here.
    let current_source = bridge.source();
    let mut model = bridge.ingress.model();
    let player = world.get_resource::<UiReadModel>()
        .map(|read| read.player.clone()).unwrap_or_default();
    model.auto_pot_view = crate::item_tooltip_query::hero_auto_pot_catalog_view(&model);
    world.insert_resource(model.clone());
    let personal = bridge.ingress.personal().cloned();
    world.insert_resource(HeroUiReadModel { hero: model, personal, player });
    let control = bridge.control.as_ref().filter(|c| bridge.control_current(c)).cloned();
    let prior_ready = !bridge.input_reset && bridge.sink_present
        && world.get_resource::<HeroUiContext>().is_some_and(|c|
            control.as_ref().is_some_and(|control| c.stamp == control.stamp()
                && c.frame_sequence == control.source.frame_sequence && c.ready))
        && world.get_resource::<AppliedHero>().and_then(|a| a.control.as_ref())
            .zip(control.as_ref()).is_some_and(|(old, next)| old.web_lease_token == next.web_lease_token);
    let mut context = HeroUiContext::default();
    if let Some(control) = control.as_ref() {
        context = HeroUiContext {
            stamp: control.stamp(), revision: control.control_revision,
            frame_sequence: control.source.frame_sequence, windows: control.windows,
            presentation: control.presentation,
            active: control.in_game && control.host_visible && current_source.is_some(),
            ready: prior_ready, input_enabled: control.input_enabled && bridge.sink_present,
            pending: control.pending, now_ms,
        };
    }
    world.insert_resource(context);
    if world.get_resource::<HeroInputQueue>().is_none() {
        world.insert_resource(HeroInputQueue::default());
    }
    world.resource_mut::<HeroInputQueue>().0.clear();
    if std::mem::take(&mut bridge.input_reset) {
        if let Some(mut state) = world.get_resource_mut::<HeroUiState>() { state.cancel(); }
    }
    if control.is_some() && !bridge.input_closed && bridge.sink_present {
        world.resource_mut::<HeroInputQueue>().0.extend(bridge.inputs.drain(..));
    } else {
        bridge.inputs.clear();
        if let Some(mut state) = world.get_resource_mut::<HeroUiState>() { state.cancel(); }
    }
    if world.get_resource::<HeroIntentQueue>().is_none() {
        world.insert_resource(HeroIntentQueue::default());
    }
    world.resource_mut::<HeroIntentQueue>().0.clear();
    let mut applied = world.get_resource_mut::<AppliedHero>().expect("Hero host installed");
    applied.source = current_source;
    applied.control = control;
    applied.receipt_frames = receipt_frames;
}


use mir2_client_bevy::crystal_ui::{
    bag_paint::{BagPaintAction, bag_cell_rect},
    character_page::CRYSTAL_CHARACTER_EQUIPMENT_SLOTS,
    hero_dialog::{geometry::{window_rect, HeroWindow}, render::HeroAction, HeroPage},
    panel_layouts::INVENTORY_GRID_ORIGIN,
    spec::CrystalRect,
};

fn near(a: f32, b: f32) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= 1.
}
fn same_rect(a: CrystalRect, b: CrystalRect) -> bool {
    near(a.left, b.left) && near(a.top, b.top)
        && near(a.width, b.width) && near(a.height, b.height)
}
/// Actual post-layout center and size, converted from physical to logical pixels.
/// Rotation/shear must not turn a rectangular hit region into a different shape.
fn logical_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> Option<CrystalRect> {
    let affine = transform.affine();
    if !node.inverse_scale_factor.is_finite() || node.inverse_scale_factor <= 0.
        || !node.size().is_finite() || node.size().min_element() <= 0.
        || !affine.translation.is_finite() || !affine.matrix2.is_finite()
        || (affine.matrix2.x_axis - Vec2::X).abs().max_element() > 0.0001
        || (affine.matrix2.y_axis - Vec2::Y).abs().max_element() > 0.0001 {
        return None;
    }
    let size = node.size() * node.inverse_scale_factor;
    let origin = (affine.translation - node.size() / 2.) * node.inverse_scale_factor;
    Some(CrystalRect::new(origin.x, origin.y, size.x, size.y))
}
/// Bijection rejects missing, extra, duplicate, misplaced or wrong-page controls.
fn rect_set_matches<K: PartialEq>(
    expected: &[(K, CrystalRect)], observed: &[(K, CrystalRect)],
) -> bool {
    if expected.len() != observed.len() { return false; }
    let mut used = vec![false; observed.len()];
    for (key, rect) in expected {
        let Some(index) = observed.iter().enumerate().position(|(index, (other, actual))|
            !used[index] && key == other && same_rect(*rect, *actual)) else { return false; };
        used[index] = true;
    }
    true
}
#[derive(Default)]
struct HeroLayoutPlan {
    panels: Vec<((), CrystalRect)>,
    actions: Vec<(HeroAction, CrystalRect)>,
    personal: Vec<((), CrystalRect)>,
    personal_cells: Vec<((u8, u32), CrystalRect)>,
    personal_actions: Vec<(BagPaintAction, CrystalRect)>,
}
fn offset(rect: CrystalRect, parent: CrystalRect) -> CrystalRect {
    CrystalRect::new(parent.left + rect.left, parent.top + rect.top, rect.width, rect.height)
}
fn expected_layout(state: &HeroUiState, read: &HeroUiReadModel) -> HeroLayoutPlan {
    let ui = &state.ui;
    let mut plan = HeroLayoutPlan::default();
    if let Some(panel) = window_rect(ui, HeroWindow::Inventory) {
        plan.panels.push(((), panel));
        plan.actions.push((HeroAction::CloseInventory, offset(CrystalRect::new(299., 2., 24., 21.), panel)));
        for local in 0..40usize {
            if ui.inventory_cell_enabled(local) {
                plan.actions.push((HeroAction::InventoryCell((local + 2) as u8),
                    offset(CrystalRect::new(14. + (local % 8) as f32 * 37.,
                        23. + (local / 8) as f32 * 33., 36., 32.), panel)));
            }
        }
        if ui.info.as_ref().is_some_and(|info| info.auto_pot) {
            for (action, rect) in [
                (HeroAction::AutoHp, CrystalRect::new(58., 206., 60., 25.)),
                (HeroAction::AutoMp, CrystalRect::new(206., 206., 60., 25.)),
                (HeroAction::AutoPotItem(true), CrystalRect::new(122., 211., 36., 32.)),
                (HeroAction::AutoPotItem(false), CrystalRect::new(166., 211., 36., 32.)),
            ] { plan.actions.push((action, offset(rect, panel))); }
        }
    }
    if let Some(panel) = window_rect(ui, HeroWindow::Character) {
        plan.panels.push(((), panel));
        plan.actions.push((HeroAction::CloseCharacter, offset(CrystalRect::new(241., 3., 24., 21.), panel)));
        for (index, page) in [HeroPage::Equipment, HeroPage::Status, HeroPage::State, HeroPage::Skills]
            .into_iter().enumerate() {
            plan.actions.push((HeroAction::Page(page),
                offset(CrystalRect::new(8. + index as f32 * 62., 70., 64., 20.), panel)));
        }
        if ui.page == HeroPage::Equipment {
            for (slot, rect) in CRYSTAL_CHARACTER_EQUIPMENT_SLOTS {
                plan.actions.push((HeroAction::EquipmentCell(slot as u8), offset(rect, panel)));
            }
        } else if ui.page == HeroPage::Skills {
            let rows = ui.info.as_ref().map_or(0, |info| info.magics.len());
            for row in ui.skill_start..rows.min(ui.skill_start.saturating_add(7)) {
                plan.actions.push((HeroAction::Skill(row),
                    offset(CrystalRect::new(52., 98. + (row - ui.skill_start) as f32 * 33., 36., 34.), panel)));
            }
            for (action, left) in [(HeroAction::Previous, 98.), (HeroAction::Next, 148.)] {
                plan.actions.push((action, offset(CrystalRect::new(left, 340., 16., 14.), panel)));
            }
        }
    }
    if let Some(panel) = window_rect(ui, HeroWindow::Belt) {
        plan.panels.push(((), panel));
        for slot in 0..2u8 {
            let rect = if ui.belt_vertical { CrystalRect::new(3., 12. + f32::from(slot) * 35., 32., 32.) }
                else { CrystalRect::new(12. + f32::from(slot) * 35., 3., 32., 32.) };
            plan.actions.push((HeroAction::InventoryCell(slot), offset(rect, panel)));
        }
        let (close, rotate) = if ui.belt_vertical {
            (CrystalRect::new(3., 82., 16., 16.), CrystalRect::new(19., 82., 16., 16.))
        } else { (CrystalRect::new(82., 19., 16., 14.), CrystalRect::new(82., 3., 16., 16.)) };
        plan.actions.push((HeroAction::BeltClose, offset(close, panel)));
        plan.actions.push((HeroAction::BeltRotate, offset(rotate, panel)));
    }
    if ui.use_confirmation.is_some() {
        plan.panels.push(((), CrystalRect::new(284., 289., 456., 190.)));
        plan.actions.push((HeroAction::UseConfirm, CrystalRect::new(544., 446., 76., 25.)));
        plan.actions.push((HeroAction::UseCancel, CrystalRect::new(644., 446., 76., 25.)));
    }
    if let Some((_, amount)) = ui.amount.as_ref() {
        plan.panels.push(((), CrystalRect::new(410., 329., 204., 109.)));
        plan.actions.push((HeroAction::AmountCancel, CrystalRect::new(590., 332., 24., 21.)));
        plan.actions.push((HeroAction::AmountCancel, CrystalRect::new(520., 405., 76., 25.)));
        if amount.amount().is_some() {
            plan.actions.push((HeroAction::AmountConfirm, CrystalRect::new(433., 405., 76., 25.)));
        }
    }
    if ui.assign.open {
        plan.panels.push(((), CrystalRect::new(322., 312., 380., 144.)));
        for index in 0..8u8 {
            plan.actions.push((HeroAction::AssignKey(index + 17), CrystalRect::new(
                339. + 32. * f32::from(index) + 5. * f32::from(index / 4), 370., 32., 32.)));
        }
        plan.actions.push((HeroAction::AssignKey(0), CrystalRect::new(606., 376., 76., 25.)));
        plan.actions.push((HeroAction::AssignSave, CrystalRect::new(606., 413., 60., 25.)));
    }
    if state.personal_open && ui.inventory_open {
        if let Some(inventory) = read.personal.as_ref() {
            let panel = CrystalRect::new(state.personal_position[0] as f32,
                state.personal_position[1] as f32, 316., 236.);
            plan.personal.push(((), panel));
            for local in 0..40usize {
                let slot = u32::from(state.personal_page) * 40 + local as u32;
                if slot >= u32::from(inventory.bag_slot_capacity()) { continue; }
                let cell = bag_cell_rect(local).expect("bounded ordinary bag page");
                let rect = CrystalRect::new(INVENTORY_GRID_ORIGIN.x as f32 + cell.left,
                    INVENTORY_GRID_ORIGIN.y as f32 + cell.top, cell.width, cell.height);
                plan.personal_cells.push(((0, slot), offset(rect, panel)));
            }
            for (action, rect) in [
                (BagPaintAction::SelectPage(0), CrystalRect::new(6., 7., 72., 23.)),
                (BagPaintAction::SelectPage(1), CrystalRect::new(76., 7., 72., 23.)),
                (BagPaintAction::Close, CrystalRect::new(289., 3., 24., 21.)),
            ] { plan.personal_actions.push((action, offset(rect, panel))); }
        }
    }
    plan
}

fn safe_integer(value: f64) -> Option<u64> {
    (value.is_finite() && value >= 0. && value <= MAX_SAFE as f64 && value.fract() == 0.)
        .then_some(value as u64)
}
fn monotonic_millis(value: f64) -> Option<u64> {
    (value.is_finite() && value >= 0. && value <= MAX_SAFE as f64)
        .then_some(value.floor() as u64)
}
fn parse_scope(raw: &str) -> Option<HeroScope> {
    if raw.is_empty() || raw.len() > 4_096 { return None; }
    serde_json::from_str::<HeroScope>(raw).ok().filter(HeroScope::valid)
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use bevy::{asset::LoadState, camera::visibility::VisibilitySystems};
    use bevy::ui::UiSystems;
    use js_sys::Function;
    use mir2_client_bevy::{
        crystal_ui::{character_materials::CrystalCharacterWingMaterials,
            hero_dialog::render::HeroRoot, hud::SharedHudSurface,
            item_image::OriginalItemImage, spec::CrystalRect},
        pending_operations::PendingLifecycleSet,
        portable_hero_ui::{appearance_materials_ready, input_regions, required_assets,
            HeroPaintSet, HeroPersonalRoot, Mir2PortableHeroUiPlugin, SharedHeroRoot},
        read_model::UiReadModelIngress,
    };
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = performance, js_name = now)]
        fn performance_now() -> f64;
    }
    thread_local! {
        static SINK: RefCell<Option<(HeroScope, u64, Function)>> = const { RefCell::new(None) };
        static STATUS: RefCell<String> = RefCell::new(String::from("{\"version\":1,\"ready\":false}"));
    }
    fn bound_sink_current(control: &HeroControl) -> bool {
        SINK.with(|sink| sink.borrow().as_ref().is_some_and(|(scope, generation, _)|
            scope == &control.source.scope && BRIDGE.with(|bridge|
                { let bridge = bridge.borrow();
                    !bridge.input_reset && bridge.feedback_current(control, *generation) })))
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Status<'a> {
        version: u32,
        source: Option<HeroSource>,
        applied_source: &'a Option<HeroSource>,
        accepted_frame_sequence: u64,
        control_revision: u64,
        sink_generation: u64,
        modal: bool,
        web_lease_token: &'a str,
        frame: u64,
        ready: bool,
        input_enabled: bool,
        input_regions: Vec<HeroRegion>,
        closed: bool,
        receipt_frames: &'a [u64],
    }
    #[derive(Serialize)]
    struct HeroRegion { left:f32,top:f32,width:f32,height:f32 }
    impl From<CrystalRect> for HeroRegion { fn from(r:CrystalRect)->Self {Self{left:r.left,top:r.top,width:r.width,height:r.height}} }
    #[derive(Resource, Default)]
    struct HeroCandidateReady(bool);
    #[derive(Resource, Default)]
    struct PublishedFrame(u64);

    pub fn install(app: &mut App) {
        app.init_resource::<AppliedHero>().init_resource::<PublishedFrame>().init_resource::<HeroCandidateReady>()
            .add_plugins(Mir2PortableHeroUiPlugin)
            .configure_sets(Update, HeroConsumeSet.after(PendingLifecycleSet::UiReset)
                .before(HeroPaintSet))
            .add_systems(Update, consume.in_set(HeroConsumeSet))
            .add_systems(Update, request_assets.after(HeroConsumeSet).before(HeroPaintSet))
            .add_systems(PostUpdate, observe_candidate.after(UiSystems::PostLayout)
                .before(VisibilitySystems::VisibilityPropagate))
            .add_systems(PostUpdate, publish.after(VisibilitySystems::VisibilityPropagate));
    }
    fn consume(world: &mut World) {
        let now = monotonic_millis(performance_now()).unwrap_or(0);
        BRIDGE.with(|bridge| apply_bridge_to_world(&mut bridge.borrow_mut(), world, now));
    }
    fn request_assets(mut applied: ResMut<AppliedHero>, read: Res<HeroUiReadModel>,
        context: Res<HeroUiContext>, server: Res<AssetServer>) {
        let paths = if context.active { required_assets(&read) } else { Vec::new() };
        if paths != applied.asset_paths {
            applied.assets = paths.iter().map(|path| server.load::<Image>(path.clone())).collect();
            applied.asset_paths = paths;
        }
    }
    fn under_root(mut entity: Entity, root: Entity, parents: &Query<&ChildOf>) -> bool {
        for _ in 0..16 {
            if entity == root { return true; }
            let Ok(parent) = parents.get(entity) else { return false; };
            entity = parent.parent();
        }
        false
    }


    fn cell_ready(
        cell: Entity, item: Option<&mir2_client_bevy::inventory::ItemModel>,
        applied: &AppliedHero,
        images: &Query<(Entity, &Node, &ComputedNode, &ImageNode), With<OriginalItemImage>>,
        texts: &Query<(Entity, &Text, &TextFont, Option<&bevy::text::TextLayoutInfo>)>,
        parents: &Query<&ChildOf>,
    ) -> bool {
        let icons = images.iter().filter(|(entity,_,_,_)|under_root(*entity,cell,parents)).collect::<Vec<_>>();
        let Some(item)=item else {return icons.is_empty();};
        let Some(index)=item.user_item_image_index() else{return false;};
        let path=format!("original-ui/Items/{index}.png");
        let Some(expected)=applied.asset_paths.iter().position(|value|value==&path)
            .and_then(|index|applied.assets.get(index)) else{return false;};
        if icons.len()!=1 || icons[0].3.image!=*expected {return false;}
        let count=item.crystal_stack_label();
        count.is_empty() || texts.iter().any(|(entity,text,_,_)|
            under_root(entity,cell,parents)&&text.0==count)
    }

    fn observe_candidate(
        applied: Res<AppliedHero>, context: Res<HeroUiContext>, read: Res<HeroUiReadModel>,
        state: Res<HeroUiState>,
        assets_state: (Res<SharedHudSurface>, Res<UiReadModelIngress>, Res<AssetServer>, Option<Res<CrystalCharacterWingMaterials>>),
        windows: Query<&Window>,
        mut roots: Query<(Entity, &SharedHeroRoot, &ComputedNode, &UiGlobalTransform, &mut Visibility)>,
        panels: Query<(Entity, &ComputedNode, &UiGlobalTransform), With<HeroRoot>>,
        personal_roots: Query<(Entity, &ComputedNode, &UiGlobalTransform), With<HeroPersonalRoot>>,
        page_images: Query<(Entity, &ImageNode, &ComputedNode, &UiGlobalTransform)>,
        item_images: Query<(Entity, &Node, &ComputedNode, &ImageNode), With<OriginalItemImage>>,
        cells: (Query<(Entity, &HeroAction, &ComputedNode, &UiGlobalTransform)>,
            Query<(Entity, &mir2_client_bevy::crystal_ui::bag_paint::BagPaintCell, &ComputedNode, &UiGlobalTransform)>,
            Query<(Entity, &BagPaintAction, &ComputedNode, &UiGlobalTransform)>),
        texts: Query<(Entity, &Text, &TextFont, Option<&bevy::text::TextLayoutInfo>)>,
        parents: Query<&ChildOf>, mut candidate: ResMut<HeroCandidateReady>,
    ) {
        let (surface,ui_ingress,server,wings)=assets_state;
        let Some(control) = applied.control.as_ref() else {
            candidate.0 = false;
            for (_, _, _, _, mut visibility) in &mut roots { *visibility = Visibility::Hidden; }
            return;
        };
        let current = bound_sink_current(control);
        let assets = !applied.assets.is_empty()
            && matches!(server.get_load_state(surface.font.id()), Some(LoadState::Loaded))
            && applied.assets.iter().all(|h|
                matches!(server.get_load_state(h.id()), Some(LoadState::Loaded)));
        let window_ok = context.presentation.zip(windows.get(surface.window).ok())
            .is_some_and(|(p,w)| w.visible && near(w.width(),p.logical_width)
                && near(w.height(),p.logical_height));
        let root_entity = roots.single().ok().map(|(e,_,_,_,_)|e);
        let layout_ok = root_entity.is_some_and(|root| {
            let root_ok = roots.get(root).ok().is_some_and(|(_,stamp,node,transform,_)|
                stamp.stamp == context.stamp && stamp.revision == context.revision
                    && stamp.frame_sequence == context.frame_sequence
                    && stamp.regions == input_regions(&context,&state,&read)
                    && context.presentation.is_some_and(|p| logical_rect(node, transform)
                        .is_some_and(|actual| same_rect(actual, CrystalRect::new(0., 0., p.logical_width, p.logical_height)))));

            let plan = expected_layout(&state, &read);
            let observed_panels = panels.iter().filter(|(entity, _, _)|
                under_root(*entity, root, &parents)).map(|(_, node, transform)|
                    logical_rect(node, transform).map(|rect| ((), rect)))
                .collect::<Option<Vec<_>>>();
            let observed_personal = personal_roots.iter().filter(|(entity, _, _)|
                under_root(*entity, root, &parents)).map(|(_, node, transform)|
                    logical_rect(node, transform).map(|rect| ((), rect)))
                .collect::<Option<Vec<_>>>();
            let (hero_cells, personal_cells, personal_actions) = &cells;
            let observed_actions = hero_cells.iter().filter(|(entity, _, _, _)|
                under_root(*entity, root, &parents)).map(|(_, action, node, transform)|
                    logical_rect(node, transform).map(|rect| (*action, rect)))
                .collect::<Option<Vec<_>>>();
            let observed_cells = personal_cells.iter().filter(|(entity, _, _, _)|
                under_root(*entity, root, &parents)).map(|(_, cell, node, transform)|
                    logical_rect(node, transform).map(|rect| ((cell.container, cell.slot), rect)))
                .collect::<Option<Vec<_>>>();
            let observed_personal_actions = personal_actions.iter()
                .filter(|(entity, action, _, _)| under_root(*entity, root, &parents)
                    && !matches!(action, BagPaintAction::InspectCell { .. }))
                .map(|(_, action, node, transform)|
                    logical_rect(node, transform).map(|rect| (*action, rect)))
                .collect::<Option<Vec<_>>>();
            if !root_ok
                || !observed_panels.is_some_and(|actual| rect_set_matches(&plan.panels, &actual))
                || !observed_personal.is_some_and(|actual| rect_set_matches(&plan.personal, &actual))
                || !observed_actions.is_some_and(|actual| rect_set_matches(&plan.actions, &actual))
                || !observed_cells.is_some_and(|actual| rect_set_matches(&plan.personal_cells, &actual))
                || !observed_personal_actions.is_some_and(|actual|
                    rect_set_matches(&plan.personal_actions, &actual)) { return false; }
            if let Some(panel) = window_rect(&state.ui, HeroWindow::Character) {
                let path = match state.ui.page {
                    HeroPage::Equipment => format!("original-ui/Prguse/{}.png",
                        if read.hero.info.as_ref().is_some_and(|i| i.gender == mir2_protocol::MirGender::Male) { 340 } else { 341 }),
                    HeroPage::Status => "original-ui/Title/506.png".into(),
                    HeroPage::State => "original-ui/Title/507.png".into(),
                    HeroPage::Skills => "original-ui/Title/508.png".into(),
                };
                let Some(handle) = applied.asset_paths.iter().position(|value| value == &path)
                    .and_then(|index| applied.assets.get(index)) else { return false; };
                let expected = offset(mir2_client_bevy::crystal_ui::panel_navigation::CHARACTER_PAGE_RECT, panel);
                if !page_images.iter().any(|(entity, image, node, transform)|
                    under_root(entity, root, &parents) && image.image == *handle
                        && logical_rect(node, transform).is_some_and(|actual| same_rect(expected, actual))) {
                    return false;
                }
            }
            for (entity,node,computed,_) in &item_images {
                if !under_root(entity,root,&parents) {continue;}
                if node.display == Display::None || computed.size().x <= 0.
                    || computed.size().y <= 0. {return false;}
            }
            for (entity,action,_,_) in hero_cells {
                if !under_root(entity,root,&parents){continue;}
                use mir2_client_bevy::crystal_ui::hero_dialog::render::HeroAction;
                let (model,container,slot)=match action {
                    HeroAction::InventoryCell(slot)=>(&read.hero.inventory_view,0,u32::from(*slot)),
                    HeroAction::EquipmentCell(slot)=>(&read.hero.inventory_view,2,u32::from(*slot)),
                    HeroAction::AutoPotItem(hp)=>(&read.hero.auto_pot_view,0,u32::from(!*hp)),
                    _=>continue,
                };
                let item=model.items.iter().find(|item|item.container==container&&item.slot==slot&&item.quantity>0);
                if !cell_ready(entity,item,&applied,&item_images,&texts,&parents){return false;}
            }
            for (entity,cell,_,_) in personal_cells {
                if !under_root(entity,root,&parents){continue;}
                let item=read.personal.as_ref().and_then(|model|model.items.iter().find(|item|
                    item.container==cell.container&&item.slot==cell.slot&&item.quantity>0));
                if cell.unique_id != item.and_then(|item| item.unique_id)
                    || !cell_ready(entity,item,&applied,&item_images,&texts,&parents){return false;}
            }
            for (entity,text,font,layout) in &texts {
                if !under_root(entity,root,&parents) || text.0.trim().is_empty() {continue;}
                if !matches!(&font.font,FontSource::Handle(handle) if handle==&surface.font)
                    || !layout.is_some_and(|layout| !layout.glyphs.is_empty()
                        && layout.size.x.is_finite() && layout.size.y.is_finite()
                        && layout.size.x>0. && layout.size.y>0.) {return false;}
            }
            true
        });
        let ready = current && context.active && applied.source.as_ref()==Some(&control.source)
            && surface.active && surface.generation==control.hud_generation
            && ui_ingress.has_complete_generation(control.hud_generation)
            && assets && window_ok && layout_ok && appearance_materials_ready(&read,wings.as_deref())
            && read.hero.inventory_view.items.iter().filter(|item|item.container==2&&item.slot<=2)
                .all(|item|item.state_image==0||item.state_image_width>0&&item.state_image_height>0)
            && (!context.windows.inventory_open || BRIDGE.with(|b|b.borrow().ingress.personal_source()
                .is_some_and(|source|source.equipment_excluded&&source.instance_metadata_complete)));
        candidate.0 = ready;
        for (_,_,_,_,mut visibility) in &mut roots {
            *visibility = if ready {Visibility::Inherited} else {Visibility::Hidden};
        }
    }
    fn publish(
        mut frame:ResMut<PublishedFrame>,applied:Res<AppliedHero>,candidate:Res<HeroCandidateReady>,
        mut context:ResMut<HeroUiContext>,mut state:ResMut<HeroUiState>,read:Res<HeroUiReadModel>,
        roots:Query<(&SharedHeroRoot,&InheritedVisibility)>,mut intents:ResMut<HeroIntentQueue>,
    ) {
        frame.0=frame.0.saturating_add(1);
        let Some(control)=applied.control.as_ref() else {
            context.ready=false;intents.0.clear();
            STATUS.with(|s|*s.borrow_mut()=serde_json::json!({"version":1,"frame":frame.0,"ready":false,"inputEnabled":false,"inputRegions":[],"appliedSource":applied.source}).to_string());
            return;
        };
        let current=bound_sink_current(control);
        let visible=roots.single().ok().is_some_and(|(root,visible)|root.stamp==context.stamp
            &&root.revision==context.revision&&root.frame_sequence==context.frame_sequence&&visible.get());
        let ready=candidate.0&&current&&visible&&applied.source.as_ref()==Some(&control.source);
        context.ready=ready;
        context.input_enabled=ready&&control.input_enabled;
        let source = BRIDGE.with(|bridge| bridge.borrow().source());
        let accepted_frame_sequence = BRIDGE.with(|bridge| bridge.borrow().ingress.frame_sequence());
        let closed = BRIDGE.with(|bridge| {
            let bridge=bridge.borrow();bridge.input_closed || bridge.ingress.status().closed
        });
        let status = Status {
            version:1, source, applied_source:&applied.source, accepted_frame_sequence,
            control_revision:control.control_revision,
            sink_generation:SINK.with(|s|s.borrow().as_ref().map_or(0,|(_,generation,_)|*generation)),
            modal:state.ui.modal(),web_lease_token:&control.web_lease_token,
            frame:frame.0,ready,input_enabled:ready&&context.input_enabled,
            input_regions:if ready {input_regions(&context,&state,&read).into_iter().map(HeroRegion::from).collect()} else {vec![]},
            closed,receipt_frames:&applied.receipt_frames,
        };
        if let Ok(json)=serde_json::to_string(&status) {STATUS.with(|s|*s.borrow_mut()=json);}
        for intent in std::mem::take(&mut intents.0) {
            if !ready || intent.stamp!=control.stamp() {continue;}
            // Never hold a RefCell borrow across arbitrary JavaScript reentry.
            let sink=SINK.with(|s|s.borrow().as_ref().cloned());
            let Some((scope,generation,function))=sink else {continue;};
            if scope!=control.source.scope
                || !BRIDGE.with(|b|b.borrow().feedback_current(control,generation)){continue;}
            let Ok(json)=serde_json::to_string(&serde_json::json!({
                "version":1,"source":control.source,"controlRevision":control.control_revision,
                "webLeaseToken":control.web_lease_token,"intent":intent,
            })) else {continue;};
            let result=function.call1(&JsValue::NULL,&JsValue::from_str(&json));
            let still_current=BRIDGE.with(|b|b.borrow().feedback_current(control,generation));
            if still_current {
                state.accepted(&intent,result.ok().and_then(|value|value.as_bool())==Some(true),
                    context.now_ms);
            }
        }
    }
    #[wasm_bindgen(js_name=getMir2HeroUiAbiVersion)]
    pub fn abi_version()->u32 {
        if cfg!(all(feature="webgl2",not(feature="webgpu"),not(feature="webgl2-shared-ui"))) {0} else {1}
    }
    #[wasm_bindgen(js_name=getMir2HeroActionBasisVersion)]
    pub fn action_basis_version()->u32 {abi_version()}
    #[wasm_bindgen(js_name=activateMir2HeroIngress)]
    pub fn activate(raw:String)->bool {
        let Some(scope)=parse_scope(&raw) else{return false;};
        BRIDGE.with(|b|b.borrow_mut().activate(scope))
    }
    #[wasm_bindgen(js_name=pushMir2HeroRawFrame)]
    pub fn raw_frame(scope:String,sequence:f64,raw:String,received_at_ms:f64)->bool {
        let (Some(scope),Some(sequence),Some(at))=(parse_scope(&scope),safe_integer(sequence),
            monotonic_millis(received_at_ms)) else{return false;};
        BRIDGE.with(|b|b.borrow_mut().push_raw(&scope,sequence,&raw,at))
    }
    #[wasm_bindgen(js_name=pushMir2HeroVerifiedOwnerFrame)]
    pub fn verified_owner_frame(scope:String,sequence:f64,raw:String,received_at_ms:f64,expected:String)->bool {
        let (Some(scope),Some(sequence),Some(at))=(parse_scope(&scope),safe_integer(sequence),
            monotonic_millis(received_at_ms)) else{return false;};
        if expected.is_empty() || expected.len() > MAX_CONTROL_BYTES { return false; }
        let Ok(expected)=serde_json::from_str::<VerifiedHeroOwner>(&expected) else{return false;};
        if !expected.valid() { return false; }
        BRIDGE.with(|b|b.borrow_mut().push_verified_owner(&scope,sequence,&raw,at,&expected))
    }
    #[wasm_bindgen(js_name=withdrawMir2HeroIngress)]
    pub fn withdraw(raw:String)->bool {
        let Some(scope)=parse_scope(&raw) else{return false;};
        let withdrawn=BRIDGE.with(|b|b.borrow_mut().withdraw(&scope));
        if withdrawn {SINK.with(|s|*s.borrow_mut()=None);}
        withdrawn
    }
    #[wasm_bindgen(js_name=getMir2HeroIngressCheckpoint)]
    pub fn checkpoint(raw:String)->Option<String> {
        let scope=parse_scope(&raw)?;
        BRIDGE.with(|b|b.borrow().checkpoint(&scope))
    }
    /// Only a held string produced by this ABI in the same browser document.
    /// The page must never route gateway payloads into this function.
    #[wasm_bindgen(js_name=restoreMir2HeroIngressCheckpoint)]
    pub fn restore(scope:String,checkpoint:String)->bool {
        let Some(scope)=parse_scope(&scope) else{return false;};
        BRIDGE.with(|b|b.borrow_mut().restore(&scope,&checkpoint))
    }
    #[wasm_bindgen(js_name=setMir2HeroUiControl)]
    pub fn control(scope:String,control:String)->bool {
        let Some(scope)=parse_scope(&scope) else{return false;};
        BRIDGE.with(|b|b.borrow_mut().set_control(&scope,&control))
    }
    #[wasm_bindgen(js_name=setMir2HeroUiInputEdge)]
    pub fn input(scope:String,edge:String)->bool {
        let Some(scope)=parse_scope(&scope) else{return false;};
        let bound=BRIDGE.with(|b|b.borrow().control.as_ref().cloned())
            .is_some_and(|control| control.source.scope==scope && bound_sink_current(&control));
        bound && BRIDGE.with(|b|b.borrow_mut().push_edge(&scope,&edge))
    }
    #[wasm_bindgen(js_name=setMir2HeroUiIntentSink)]
    pub fn set_sink(scope:String,function:Function)->f64 {
        let Some(scope)=parse_scope(&scope) else{return 0.;};
        let Some(generation)=BRIDGE.with(|b|b.borrow_mut().advance_sink(&scope)) else{return 0.;};
        SINK.with(|s|*s.borrow_mut()=Some((scope,generation,function)));
        generation as f64
    }
    #[wasm_bindgen(js_name=clearMir2HeroUiIntentSink)]
    pub fn clear_sink(scope:String,expected_generation:f64)->bool {
        let (Some(scope),Some(generation))=(parse_scope(&scope),safe_integer(expected_generation)) else{return false;};
        if !BRIDGE.with(|b|b.borrow_mut().clear_sink(&scope,generation)){return false;}
        SINK.with(|s|*s.borrow_mut()=None);
        true
    }
    #[wasm_bindgen(js_name=getMir2HeroUiStatus)]
    pub fn status()->String {
        // Source metadata is immediately queryable before any control or frame.
        let old=STATUS.with(|s|s.borrow().clone());
        let mut value=serde_json::from_str::<serde_json::Value>(&old).unwrap_or_else(|_|serde_json::json!({"version":1,"ready":false}));
        let sink_key=SINK.with(|s|s.borrow().as_ref().map(|(scope,generation,_)|(scope.clone(),*generation)));
        BRIDGE.with(|b| {
            let b=b.borrow();
            value["source"]=serde_json::json!(b.source());
            value["acceptedFrameSequence"]=serde_json::json!(b.ingress.frame_sequence());
            value["closed"]=serde_json::json!(b.input_closed||b.ingress.status().closed);
            let matching=!b.input_reset && b.control.as_ref().is_some_and(|c|sink_key.as_ref()
                .is_some_and(|(scope,generation)| scope==&c.source.scope && b.feedback_current(c,*generation))
                &&value["controlRevision"].as_u64()==Some(c.control_revision)
                &&value["webLeaseToken"].as_str()==Some(c.web_lease_token.as_str())
                &&serde_json::from_value::<HeroSource>(value["appliedSource"].clone()).ok().as_ref()==Some(&c.source));
            if !matching {value["ready"]=serde_json::json!(false);value["inputEnabled"]=serde_json::json!(false);value["inputRegions"]=serde_json::json!([]);}
        });
        value.to_string()
    }
}
#[cfg(target_arch="wasm32")]
pub use web::install;


#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{HeroUserInformation, MirClass, MirGender};
    use serde_json::{json, Value};

    fn scope(run:u64)->HeroScope {HeroScope {run_generation:run,connection_generation:1,
        session_generation:1,scene_revision:1,player_object_id:42,map_file_name:"TestMap".into()}}
    fn owner()->Value {
        json!({"playerObjectId":42,"mapFileName":"TestMap","inventoryCapacity":46,"maxBagSlots":40,
            "gold":7,"inventoryItems":[],"beltItems":[],"equipmentItems":[],
            "heroMaxExperience":200,"heroVitals":{"hp":20,"maxHp":30,"mp":10,"maxMp":15},
            "heroStats":[],"heroWeights":{"bag":1,"wear":2,"hand":3},"heroInventoryCapacity":10,
            "heroInventoryItems":[],"heroEquipmentItems":[],"stage5Systems":{"heroLearnedMagics":[],
                "hero":{"name":"Hero","class":"Warrior","gender":"Male","level":2,"experience":77,
                "behaviour":0,"spawned":true,"autoPot":false,"autoHpPercent":30,"autoMpPercent":40,
                "hpItemIndex":0,"mpItemIndex":0}}})
    }
    fn information()->HeroUserInformation {
        HeroUserInformation {object_id:12,name:"Hero".into(),class:MirClass::Warrior,gender:MirGender::Male,
            level:2,hair:3,hp:20,mp:10,experience:77,max_experience:200,inventory:Some(vec![None;10]),
            equipment:Some(vec![None;14]),magics:vec![],auto_pot:false,auto_hp_percent:30,auto_mp_percent:40,
            hp_item_index:0,mp_item_index:0}
    }
    fn start()->(HeroBridge,HeroScope) {
        let mut bridge=HeroBridge::default();let scope=scope(1);assert!(bridge.activate(scope.clone()));
        assert!(bridge.push_raw(&scope,1,&json!({"type":"worldSnapshot","payload":owner()}).to_string(),100));
        assert!(bridge.source().is_none());
        assert!(bridge.push_raw(&scope,2,&json!({"type":"packet","packet":"HeroInformation",
            "payload":{"info":information()}}).to_string(),101));
        assert!(bridge.source().is_some());(bridge,scope)
    }
    fn control(source:&HeroSource,revision:u64)->String {
        json!({"source":source,"controlRevision":revision,"webLeaseToken":"web-authority:5:hero:19",
            "hudGeneration":8,"webModelRevision":9,"presentationRevision":10,"windowEpochs":[1,2,3],
            "windows":{"inventoryOpen":true,"characterOpen":false,"characterPage":"equipment",
                "beltVisible":false,"beltVertical":false},
            "presentation":{"logicalWidth":1024,"logicalHeight":768,"stageCssScale":1,"touch":false},
            "inGame":true,"hostVisible":true,"inputEnabled":true,"pending":false}).to_string()
    }
    fn edge(control:&HeroControl,sequence:u64)->String {
        json!({"sinkGeneration":2,"controlRevision":control.control_revision,"webLeaseToken":control.web_lease_token,
            "edge":{"stamp":control.stamp(),"sequence":sequence,"pointerId":7,"phase":"down",
            "x":304,"y":8,"button":0,"key":"","text":"","control":false,"shift":false}}).to_string()
    }
    fn world()->World {
        let mut world=World::new();
        world.init_resource::<AppliedHero>();world.init_resource::<UiReadModel>();
        world.init_resource::<HeroUiContext>();world.init_resource::<HeroUiState>();
        world
    }

    #[test]
    fn hero_host_verified_owner_is_display_source_without_core_or_ledger_side_effects() {
        let (mut bridge,scope)=start();let before=bridge.source().unwrap();
        let expected=VerifiedHeroOwner {request_id:u64::MAX.to_string(),actor:"1".repeat(64),
            producer_scope:"2".repeat(64),server_revision:"0".into()};
        let mut next=owner();next["heroVitals"]["hp"]=json!(7);
        let raw=json!({"type":"npcPurchaseOwner","protocolVersion":1,"requestId":expected.request_id,
            "reply":{"kind":"producer","producer":{"actor":expected.actor,
                "producerScope":expected.producer_scope,"serverRevision":expected.server_revision}},
            "snapshot":next,"authority":{"actor":expected.actor,
                "producerScope":expected.producer_scope,"serverRevision":expected.server_revision}}).to_string();
        let mut wrong=expected.clone();wrong.actor="3".repeat(64);
        assert!(!bridge.push_verified_owner(&scope,3,&raw,102,&wrong));
        assert_eq!(bridge.source(),Some(before.clone()));assert_eq!(bridge.ingress.frame_sequence(),2);
        assert!(bridge.push_verified_owner(&scope,3,&raw,102,&expected));
        let source=bridge.source().unwrap();assert_eq!(source.frame_sequence,3);
        assert!(source.rust_model_revision>before.rust_model_revision);
        assert_eq!(source.hero_object_id,before.hero_object_id);
        assert_eq!(bridge.ingress.status().receipt_count,0);
        let mut world=world();apply_bridge_to_world(&mut bridge,&mut world,103);
        assert_eq!(world.resource::<HeroUiReadModel>().hero.info.as_ref().unwrap().hp,7);
        assert!(world.resource::<HeroUiReadModel>().hero.skill_key_ack.is_none());
        assert_eq!(world.resource::<AppliedHero>().receipt_frames.len(),0);
        let mut absent:Value=serde_json::from_str(&raw).unwrap();
        absent["snapshot"]["stage5Systems"]["hero"]=Value::Null;
        absent["snapshot"]["heroMaxExperience"]=Value::Null;
        assert!(bridge.push_verified_owner(&scope,4,&absent.to_string(),104,&expected));
        assert!(bridge.ingress.available());assert!(bridge.source().is_none());
        apply_bridge_to_world(&mut bridge,&mut world,105);
        assert!(world.resource::<HeroUiReadModel>().hero.info.is_none());
        assert!(world.resource::<AppliedHero>().source.is_none());
        assert!(!world.resource::<HeroUiContext>().ready);
        assert_eq!(world.resource::<AppliedHero>().receipt_frames.len(),0);
    }

    #[test]
    fn hero_host_separates_rust_source_from_web_counters_and_ignored_raw_cursor() {
        let (mut bridge,scope)=start();let source=bridge.source().unwrap();
        assert_ne!(source.hero_generation,19);
        assert!(bridge.set_control(&scope,&control(&source,1)));
        let actual=bridge.control.as_ref().unwrap();
        assert_eq!(actual.web_model_revision,9);
        assert_eq!(actual.stamp().hero_generation,source.hero_generation);
        assert!(bridge.push_raw(&scope,3,r#"{"type":"packet","packet":"Unrelated","payload":{}}"#,102));
        assert_eq!(bridge.ingress.frame_sequence(),3);assert_eq!(bridge.source(),Some(source));
        assert!(bridge.control_current(bridge.control.as_ref().unwrap()));
    }
    #[test]
    fn hero_host_stale_control_and_duplicate_json_do_not_poison_current_owner() {
        let (mut bridge,scope)=start();let source=bridge.source().unwrap();
        let valid=control(&source,1);assert!(bridge.set_control(&scope,&valid));
        let mut stale:Value=serde_json::from_str(&control(&source,2)).unwrap();
        stale["source"]["rustModelRevision"]=json!(source.rust_model_revision+1);
        assert!(!bridge.set_control(&scope,&stale.to_string()));
        let duplicate=valid.replacen("\"controlRevision\":1","\"controlRevision\":2,\"controlRevision\":3",1);
        assert!(!bridge.set_control(&scope,&duplicate));
        assert!(bridge.control_current(bridge.control.as_ref().unwrap()));
        assert_eq!(bridge.control_high,1);
    }
    #[test]
    fn hero_host_old_cleanup_cannot_retire_replacement_run_or_sink() {
        let (mut bridge,old)=start();let old_sink=bridge.advance_sink(&old).unwrap();
        let new=scope(2);assert!(bridge.activate(new.clone()));
        let new_sink=bridge.advance_sink(&new).unwrap();assert!(new_sink>old_sink);
        assert!(!bridge.withdraw(&old));assert!(bridge.advance_sink(&old).is_none());
        assert!(bridge.ingress.is_current(&new));assert_eq!(bridge.sink_generation,new_sink);
        assert!(!bridge.activate(old));
    }
    #[test]
    fn hero_host_bounded_input_overflow_keeps_accepted_fifo_for_one_world_consumer() {
        let (mut bridge,scope)=start();
        assert!(bridge.push_raw(&scope,3,&json!({"type":"packet","packet":"UseItem",
            "payload":{"uniqueId":u64::MAX,"grid":"HeroInventory","success":false}}).to_string(),102));
        assert_eq!(bridge.ingress.status().receipt_count,1);
        let source=bridge.source().unwrap();
        assert!(bridge.set_control(&scope,&control(&source,1)));
        let control=bridge.control.as_ref().unwrap().clone();
        assert!(bridge.advance_sink(&scope).is_some());
        for sequence in 1..=MAX_INPUTS as u64 {assert!(bridge.push_edge(&scope,&edge(&control,sequence)));}
        assert!(!bridge.push_edge(&scope,&edge(&control,MAX_INPUTS as u64+1)));
        assert!(bridge.input_closed);assert!(bridge.inputs.is_empty());assert!(!bridge.activate(scope.clone()));
        assert!(bridge.ingress.status().active);
        let mut world=world();apply_bridge_to_world(&mut bridge,&mut world,103);
        assert!(world.resource::<HeroInputQueue>().0.is_empty());assert!(!world.resource::<HeroUiContext>().active);
        assert_eq!(world.resource::<AppliedHero>().receipt_frames,vec![3]);
        assert_eq!(bridge.ingress.status().receipt_count,0);
    }
    #[test]
    fn hero_host_consumer_reapplies_current_projection_after_unscoped_world_reset() {
        let (mut bridge,scope)=start();let source=bridge.source().unwrap();
        assert!(bridge.set_control(&scope,&control(&source,1)));
        let mut world=world();apply_bridge_to_world(&mut bridge,&mut world,102);
        assert_eq!(world.resource::<HeroModel>().info.as_ref().unwrap().object_id,12);
        world.insert_resource(HeroModel::default());
        world.insert_resource(HeroUiReadModel::default());
        apply_bridge_to_world(&mut bridge,&mut world,103);
        assert_eq!(world.resource::<HeroModel>().info.as_ref().unwrap().object_id,12);
        assert_eq!(world.resource::<AppliedHero>().source,Some(source));
        assert!(world.resource::<HeroUiContext>().active);
        assert!(!world.resource::<HeroUiContext>().ready);
        assert!(bridge.ingress.is_current(&scope));
    }
    #[test]
    fn hero_host_receipts_apply_in_fifo_before_latest_without_an_unbounded_world_queue() {
        let (mut bridge,scope)=start();let mut world=world();apply_bridge_to_world(&mut bridge,&mut world,102);
        for (sequence,id) in [(3,u64::MAX),(4,u64::MAX-1)] {
            assert!(bridge.push_raw(&scope,sequence,&json!({"type":"packet","packet":"UseItem",
                "payload":{"uniqueId":id,"grid":"HeroInventory","success":false}}).to_string(),sequence+100));
        }
        assert!(bridge.push_raw(&scope,5,&json!({"type":"worldSnapshot","payload":owner()}).to_string(),105));
        assert_eq!(bridge.ingress.status().receipt_count,2);
        assert!(bridge.checkpoint(&scope).is_none());
        apply_bridge_to_world(&mut bridge,&mut world,106);
        assert_eq!(world.resource::<AppliedHero>().receipt_frames,vec![3,4]);
        assert_eq!(bridge.ingress.status().receipt_count,0);
        assert_eq!(world.resource::<HeroModel>().item_result_serial,2);
        assert!(world.get_resource::<mir2_client_bevy::hero_model::HeroModelReceipts>().is_none());
        assert!(bridge.checkpoint(&scope).is_some());
    }
    #[test]
    fn hero_host_checkpoint_is_held_string_and_new_run_restores_before_live_input() {
        let (mut old,scope)=start();
        assert!(old.push_raw(&scope,3,r#"{"type":"packet","packet":"Unrelated","payload":{}}"#,102));
        let mut old_world=world();apply_bridge_to_world(&mut old,&mut old_world,103);
        let checkpoint=old.checkpoint(&scope).unwrap();let new_scope=super::tests::scope(2);
        let mut next=HeroBridge::default();assert!(next.activate(new_scope.clone()));
        assert!(next.restore(&new_scope,&checkpoint));assert!(next.source().is_some());
        let source=next.source().unwrap();assert_eq!(source.scope,new_scope);
        assert_eq!(source.frame_sequence,2);
        assert_eq!(next.ingress.frame_sequence(),3);
        assert_eq!(serde_json::to_value(next.ingress.model().magic_clocks).unwrap(),
            serde_json::to_value(old.ingress.model().magic_clocks).unwrap());
        assert!(!next.withdraw(&scope));
        let mut wrong=new_scope.clone();wrong.map_file_name="OtherMap".into();
        assert!(!next.restore(&wrong,&checkpoint));assert_eq!(next.source(),Some(source));
    }
    #[test]
    fn hero_host_reentrant_scope_or_sink_change_invalidates_acceptance_feedback() {
        let (mut bridge,scope)=start();let source=bridge.source().unwrap();
        assert!(bridge.set_control(&scope,&control(&source,1)));
        let captured=bridge.control.as_ref().unwrap().clone();let sink=bridge.advance_sink(&scope).unwrap();
        assert!(bridge.feedback_current(&captured,sink));
        assert!(bridge.advance_sink(&scope).is_some());assert!(!bridge.feedback_current(&captured,sink));
        let new_sink=bridge.sink_generation;
        assert!(bridge.set_control(&scope,&control(&source,2)));
        assert!(!bridge.feedback_current(&captured,new_sink));
        assert!(bridge.activate(super::tests::scope(2)));
        assert!(!bridge.feedback_current(&captured,new_sink));
    }

    #[test]
    fn hero_host_same_stamp_new_web_lease_cancels_queued_and_held_old_input() {
        let (mut bridge, scope) = start();
        let source = bridge.source().unwrap();
        assert!(bridge.advance_sink(&scope).is_some());
        assert!(bridge.set_control(&scope, &control(&source, 1)));
        let first = bridge.control.as_ref().unwrap().clone();
        let mut world = world();
        apply_bridge_to_world(&mut bridge, &mut world, 102);
        let mut context = world.resource::<HeroUiContext>().clone();
        context.ready = true;
        let read = world.resource::<HeroUiReadModel>().clone();
        let mut state = world.remove_resource::<HeroUiState>().unwrap();
        state.reconcile(&context, &read);
        let mut intents = HeroIntentQueue::default();
        let down = serde_json::from_str::<HostInput>(&edge(&first, 1)).unwrap().edge;
        assert!(state.process(&context, &read, down, &mut intents));
        assert_eq!(state.ui.armed, Some(HeroAction::CloseInventory));
        world.insert_resource(state);
        assert!(bridge.push_edge(&scope, &edge(&first, 2)));
        let mut next: Value = serde_json::from_str(&control(&source, 2)).unwrap();
        next["webLeaseToken"] = json!("web-authority:5:hero:20");
        assert!(bridge.set_control(&scope, &next.to_string()));
        assert_eq!(bridge.control.as_ref().unwrap().stamp(), first.stamp());
        assert!(bridge.inputs.is_empty());
        assert!(!bridge.push_edge(&scope, &edge(&first, 3)));
        apply_bridge_to_world(&mut bridge, &mut world, 103);
        assert_eq!(world.resource::<HeroUiState>().ui.armed, None);
        assert!(!world.resource::<HeroUiContext>().ready);
        let mut up = serde_json::from_str::<HostInput>(&edge(bridge.control.as_ref().unwrap(), 4)).unwrap().edge;
        up.phase = "up".into();
        context = world.resource::<HeroUiContext>().clone();
        context.ready = true;
        assert!(!world.resource_mut::<HeroUiState>().process(&context, &read, up, &mut intents));
        assert!(intents.0.is_empty());
    }
    #[test]
    fn hero_host_clearing_current_sink_revokes_input_but_old_cleanup_cannot() {
        let (mut bridge, scope) = start();
        let source = bridge.source().unwrap();
        assert!(bridge.set_control(&scope, &control(&source, 1)));
        let control = bridge.control.as_ref().unwrap().clone();
        assert!(!bridge.push_edge(&scope, &edge(&control, 1)));
        let sink = bridge.advance_sink(&scope).unwrap();
        assert!(bridge.push_edge(&scope, &edge(&control, 1)));
        assert!(bridge.clear_sink(&scope, sink));
        assert!(bridge.inputs.is_empty());
        assert!(!bridge.feedback_current(&control, sink));
        assert!(!bridge.push_edge(&scope, &edge(&control, 2)));
        let mut world = world();
        apply_bridge_to_world(&mut bridge, &mut world, 102);
        assert!(!world.resource::<HeroUiContext>().ready);
        assert!(!world.resource::<HeroUiContext>().input_enabled);
        assert!(world.resource::<HeroInputQueue>().0.is_empty());
        let replacement = super::tests::scope(2);
        assert!(bridge.activate(replacement.clone()));
        let new_sink = bridge.advance_sink(&replacement).unwrap();
        assert!(!bridge.clear_sink(&scope, new_sink));
        assert!(bridge.sink_present);
        assert_eq!(bridge.sink_generation, new_sink);
    }
    #[test]
    fn hero_host_actual_layout_rejects_missing_duplicate_misplaced_and_wrong_page_cells() {
        let (mut bridge, scope) = start();
        let source = bridge.source().unwrap();
        assert!(bridge.set_control(&scope, &control(&source, 1)));
        let mut world = world();
        apply_bridge_to_world(&mut bridge, &mut world, 102);
        let read = world.resource::<HeroUiReadModel>().clone();
        let mut state = HeroUiState::default();
        let mut context = world.resource::<HeroUiContext>().clone();
        context.windows.character_open = true;
        state.reconcile(&context, &read);
        let plan = expected_layout(&state, &read);
        assert!(plan.actions.iter().any(|(action, _)| matches!(action, HeroAction::InventoryCell(2))));
        assert_eq!(plan.personal_cells.len(), 40);
        assert!(rect_set_matches(&plan.actions, &plan.actions));
        let mut missing = plan.actions.clone();
        missing.remove(1);
        assert!(!rect_set_matches(&plan.actions, &missing));
        let mut duplicate = plan.actions.clone();
        duplicate[1] = duplicate[0];
        assert!(!rect_set_matches(&plan.actions, &duplicate));
        let mut moved = plan.personal_cells.clone();
        moved[0].1.left += 3.;
        assert!(!rect_set_matches(&plan.personal_cells, &moved));
        let mut wrong_page = plan.personal_cells.clone();
        wrong_page[0].0.1 += 40;
        assert!(!rect_set_matches(&plan.personal_cells, &wrong_page));
        state.ui.page = HeroPage::Skills;
        assert!(!rect_set_matches(&plan.actions, &expected_layout(&state, &read).actions));
        let node = ComputedNode { size: Vec2::new(40., 20.), inverse_scale_factor: 0.5, ..default() };
        let transform = UiGlobalTransform::from_xy(100., 50.);
        assert!(same_rect(logical_rect(&node, &transform).unwrap(), CrystalRect::new(40., 20., 20., 10.)));
        assert!(logical_rect(&node, &UiGlobalTransform::from_scale(Vec2::new(2., 1.))).is_none());
        let mut nan_transform = transform.affine();
        nan_transform.matrix2.x_axis.x = f32::NAN;
        assert!(logical_rect(&node, &UiGlobalTransform::from(nan_transform)).is_none());
        let bad = ComputedNode { size: Vec2::new(f32::NAN, 20.), ..node };
        assert!(logical_rect(&bad, &transform).is_none());
        state.ui.inventory_open = false;
        let closed = expected_layout(&state, &read);
        assert!(closed.personal.is_empty());
        assert!(closed.personal_cells.is_empty());
        assert!(closed.personal_actions.is_empty());
    }
    #[test]
    fn hero_host_checkpoint_keeps_first_packet_clock_across_ignored_frames() {
        let (mut old, scope) = start();
        let mut info = information();
        info.magics.push(serde_json::from_value(json!({
            "name":"Fire Ball","spell":"FireBall","base_cost":1,"level_cost":0,"icon":1,
            "level1":1,"level2":2,"level3":3,"need1":1,"need2":2,"need3":3,
            "level":1,"key":17,"experience":0,"delay":3400,"range":8,"cast_time":-1000
        })).unwrap());
        assert!(old.push_raw(&scope, 3, &json!({"type":"packet","packet":"HeroInformation",
            "payload":{"info":info}}).to_string(), 1000));
        assert!(old.push_raw(&scope, 4, r#"{"type":"packet","packet":"Unrelated","payload":{}}"#, 1100));
        let mut world = world();
        apply_bridge_to_world(&mut old, &mut world, 1200);
        let checkpoint = old.checkpoint(&scope).unwrap();
        let new_scope = super::tests::scope(2);
        let mut next = HeroBridge::default();
        assert!(next.activate(new_scope.clone()));
        assert!(next.restore(&new_scope, &checkpoint));
        assert_eq!(next.source_sequence, 3);
        assert_eq!(next.ingress.frame_sequence(), 4);
        let clock = next.ingress.model().magic_clocks[0].clone();
        assert_eq!(clock.received_ms, 1000);
        assert_eq!(clock.remaining_ms(1200), 2200);
        let mut invalid: Value = serde_json::from_str(&checkpoint).unwrap();
        invalid["sourceSequence"] = json!(5);
        let mut fresh = HeroBridge::default();
        assert!(fresh.activate(new_scope.clone()));
        assert!(!fresh.restore(&new_scope, &invalid.to_string()));
        assert_eq!(fresh.ingress.frame_sequence(), 0);
        assert!(fresh.restore(&new_scope, &checkpoint));
    }


    #[test]
    fn hero_host_old_same_scope_sink_cleanup_and_input_cannot_touch_replacement_binding() {
        let (mut bridge, scope) = start();
        let source = bridge.source().unwrap();
        assert!(bridge.set_control(&scope, &control(&source, 1)));
        let control = bridge.control.as_ref().unwrap().clone();
        let old_sink = bridge.advance_sink(&scope).unwrap();
        assert!(bridge.push_edge(&scope, &edge(&control, 1)));
        let next_sink = bridge.advance_sink(&scope).unwrap();
        assert!(next_sink > old_sink);
        assert!(bridge.inputs.is_empty());
        assert!(!bridge.clear_sink(&scope, old_sink));
        assert!(bridge.sink_present);
        assert_eq!(bridge.sink_generation, next_sink);
        assert!(!bridge.feedback_current(&control, old_sink));
        assert!(bridge.feedback_current(&control, next_sink));
        assert!(!bridge.push_edge(&scope, &edge(&control, 2)));
        let mut current: Value = serde_json::from_str(&edge(&control, 2)).unwrap();
        current["sinkGeneration"] = json!(next_sink);
        assert!(bridge.push_edge(&scope, &current.to_string()));
        assert!(bridge.clear_sink(&scope, next_sink));
        assert!(!bridge.feedback_current(&control, next_sink));
    }
    #[test]
    fn hero_host_disabled_control_cancels_old_press_even_without_an_intervening_world_frame() {
        let (mut bridge, scope) = start();
        let source = bridge.source().unwrap();
        assert!(bridge.advance_sink(&scope).is_some());
        assert!(bridge.set_control(&scope, &control(&source, 1)));
        let first = bridge.control.as_ref().unwrap().clone();
        let mut world = world();
        apply_bridge_to_world(&mut bridge, &mut world, 102);
        let read = world.resource::<HeroUiReadModel>().clone();
        let mut context = world.resource::<HeroUiContext>().clone();
        context.ready = true;
        let mut state = world.remove_resource::<HeroUiState>().unwrap();
        state.reconcile(&context, &read);
        let mut intents = HeroIntentQueue::default();
        let down = serde_json::from_str::<HostInput>(&edge(&first, 1)).unwrap().edge;
        assert!(state.process(&context, &read, down, &mut intents));
        world.insert_resource(state);
        assert!(bridge.push_edge(&scope, &edge(&first, 2)));
        let mut disabled: Value = serde_json::from_str(&control(&source, 2)).unwrap();
        disabled["inputEnabled"] = json!(false);
        assert!(bridge.set_control(&scope, &disabled.to_string()));
        assert!(bridge.inputs.is_empty());
        assert!(bridge.set_control(&scope, &control(&source, 3)));
        assert_eq!(bridge.control.as_ref().unwrap().stamp(), first.stamp());
        assert!(bridge.input_reset);
        apply_bridge_to_world(&mut bridge, &mut world, 103);
        assert_eq!(world.resource::<HeroUiState>().ui.armed, None);
        assert!(!world.resource::<HeroUiContext>().ready);
        context = world.resource::<HeroUiContext>().clone();
        context.ready = true;
        let mut up = serde_json::from_str::<HostInput>(&edge(bridge.control.as_ref().unwrap(), 3)).unwrap().edge;
        up.phase = "up".into();
        assert!(!world.resource_mut::<HeroUiState>().process(&context, &read, up, &mut intents));
        assert!(intents.0.is_empty());
    }

    #[test]
    fn hero_host_clock_and_scope_boundary_refuse_unsafe_values() {
        assert_eq!(safe_integer(1.5),None);assert_eq!(safe_integer(f64::NAN),None);
        assert_eq!(safe_integer(MAX_SAFE as f64+1.),None);assert_eq!(safe_integer(-1.),None);
        assert_eq!(monotonic_millis(100.75),Some(100));assert_eq!(monotonic_millis(f64::INFINITY),None);
        let valid=serde_json::to_string(&scope(1)).unwrap();assert!(parse_scope(&valid).is_some());
        let duplicate=valid.replacen("\"runGeneration\":1","\"runGeneration\":1,\"runGeneration\":2",1);
        assert!(parse_scope(&duplicate).is_none());
    }
}
