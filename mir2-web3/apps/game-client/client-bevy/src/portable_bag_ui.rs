//! Browser host adapter for the native Crystal bag painter.
//!
//! This tree owns only presentation and instance-pinned intentions. The page
//! owns the Gateway socket and the equipment reservation ledger.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::text::{FontSource, LineBreak};
use bevy::ui::{FocusPolicy, UiTargetCamera};

use crate::bag_ui::{resolve_exact, BagSelection};
use crate::crystal_ui::bag_paint::{
    paint_crystal_inventory_with_layout, BagCellPolicy, BagPaintAction, BagPaintCell,
    BagPaintGridViewport, BagPaintLayout, BagPaintOptions,
};
use crate::crystal_ui::item_image::{layout_original_item_images, OriginalItemImage};
use crate::crystal_ui::item_tooltip::{CrystalItemTooltipDocument, CrystalItemTooltipLine, CrystalItemTooltipSection};
use crate::crystal_ui::widget::{spawn_crystal_item_hint_document, CrystalItemHint,
    CrystalItemHintOverlayText, CRYSTAL_HINT_Z_INDEX};
use crate::inventory::InventoryModel;
use crate::portable_quest_ui::{QuestUiFont, QuestUiTargetCamera};
use crate::pending_operations::PendingLifecycleSet;
use crate::read_model::PlayerStats;

pub use crate::crystal_ui::bag_paint::PORTABLE_BAG_REQUIRED_SKINS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BagIdentity {
    pub run_generation: u64,
    pub connection_generation: u64,
    pub session_generation: u64,
    pub owner_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagPage {
    Bag1,
    Bag2,
    Quest,
}

impl BagPage {
    pub fn paint_index(self) -> u8 {
        match self {
            Self::Bag1 => 0,
            Self::Bag2 => 1,
            Self::Quest => 2,
        }
    }

    pub fn from_paint_index(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Bag1),
            1 => Some(Self::Bag2),
            2 => Some(Self::Quest),
            _ => None,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct BagUiHostContext {
    pub identity: BagIdentity,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub bag_open: bool,
    pub page: BagPage,
    pub input_enabled: bool,
    pub presentation_ready: bool,
    pub presentation: Option<BagUiPresentation>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BagUiPresentation {
    pub logical_width: f32,
    pub logical_height: f32,
    pub stage_css_scale: f32,
    pub touch: bool,
}

impl BagUiPresentation {
    pub fn paint_layout(self) -> BagPaintLayout {
        if self.touch {
            BagPaintLayout { scale: 1.28 / self.stage_css_scale, touch: true }
        } else {
            BagPaintLayout::default()
        }
    }
}

impl Default for BagUiHostContext {
    fn default() -> Self {
        Self {
            identity: BagIdentity::default(),
            model_revision: 0,
            presentation_revision: 0,
            bag_open: false,
            page: BagPage::Bag1,
            input_enabled: false,
            presentation_ready: false,
            presentation: None,
        }
    }
}

/// Separate from the runtime's legacy InventoryModel and PendingOperations.
#[derive(Resource, Default, Clone)]
pub struct BagUiReadModel {
    pub inventory: InventoryModel,
    pub player: PlayerStats,
    pub blocked_unique_ids: HashSet<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagUiIntentKind {
    UseItem { source: BagSelection },
    EquipItem { source: BagSelection },
    MoveItem { source: BagSelection, target_slot: u32 },
    Close,
    SelectPage(BagPage),
    HandoffFullInventory,
    HandoffDelete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagUiIntent {
    pub identity: BagIdentity,
    pub intent_sequence: u64,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub kind: BagUiIntentKind,
}

#[derive(Resource, Default)]
pub struct BagUiIntentQueue {
    next_sequence: u64,
    intents: Vec<BagUiIntent>,
}

impl BagUiIntentQueue {
    pub fn push(&mut self, context: &BagUiHostContext, kind: BagUiIntentKind) -> bool {
        if self.intents.len() >= 64 || self.next_sequence >= 9_007_199_254_740_991 {
            return false;
        }
        self.next_sequence += 1;
        self.intents.push(BagUiIntent {
            identity: context.identity,
            intent_sequence: self.next_sequence,
            model_revision: context.model_revision,
            presentation_revision: context.presentation_revision,
            kind,
        });
        true
    }

    pub fn drain(&mut self) -> Vec<BagUiIntent> {
        std::mem::take(&mut self.intents)
    }

    pub fn clear(&mut self) {
        self.intents.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagPointerPhase {
    Down,
    Move,
    Up,
    Cancel,
    Blur,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagPointerOrigin {
    Bag,
    World,
}

#[derive(Debug, Clone, Copy)]
pub struct BagUiPointerEdge {
    pub identity: BagIdentity,
    pub sequence: u64,
    pub presentation_revision: u64,
    pub pointer_id: u64,
    pub phase: BagPointerPhase,
    pub origin: BagPointerOrigin,
    pub x: f32,
    pub y: f32,
    pub button: u8,
}

#[derive(Resource, Default)]
pub struct BagUiPointerEdges(pub Vec<BagUiPointerEdge>);

#[derive(Resource, Default)]
pub struct BagUiState {
    pub page: BagPage,
    pub selected: Option<BagSelection>,
    pub selected_quest_slot: Option<u32>,
    pub move_source: Option<BagSelection>,
    detail: Option<BagDetailState>,
    last_identity: Option<BagIdentity>,
    last_host_page: Option<BagPage>,
    last_open: bool,
    last_model_revision: u64,
    last_presentation_revision: u64,
}

impl Default for BagPage {
    fn default() -> Self {
        Self::Bag1
    }
}

impl BagUiState {
    /// Status-only ownership signal; the detail document and selection stay local.
    pub fn detail_active(&self) -> bool { self.detail.is_some() }

    fn apply_context(&mut self, context: &BagUiHostContext, model: &BagUiReadModel) -> bool {
        let identity_changed = self.last_identity != Some(context.identity);
        let page_changed = self.last_host_page != Some(context.page);
        let closed = self.last_open && !context.bag_open;
        let layout_changed = self.last_presentation_revision != context.presentation_revision;
        if identity_changed || page_changed || closed || layout_changed || !context.input_enabled {
            self.selected = None;
            self.selected_quest_slot = None;
            self.move_source = None;
            self.detail = None;
        }
        if !context.presentation.is_some_and(|presentation| presentation.touch) {
            self.detail = None;
        }
        if identity_changed || page_changed {
            self.page = context.page;
        }
        if self.page == BagPage::Bag2 && !model.inventory.second_bag_unlocked() {
            self.page = BagPage::Bag1;
        }
        if self.last_model_revision != context.model_revision {
            self.detail = None;
            self.selected_quest_slot = None;
            if self.selected.is_some_and(|selection| resolve_exact(&model.inventory, selection).is_none()) {
                self.selected = None;
            }
            if self.move_source.is_some_and(|selection| resolve_exact(&model.inventory, selection).is_none()) {
                self.move_source = None;
            }
        }
        self.last_identity = Some(context.identity);
        self.last_host_page = Some(context.page);
        self.last_open = context.bag_open;
        self.last_model_revision = context.model_revision;
        self.last_presentation_revision = context.presentation_revision;
        identity_changed || page_changed || closed || layout_changed
    }
}

#[derive(Component)]
pub struct BagUiRoot;
#[derive(Component)]
pub struct BagUiPanel;
#[derive(Component)]
pub struct BagUiInspector;
#[derive(Component)]
pub struct BagUiDetail;
#[derive(Component)]
struct BagUiDetailBody;
/// Attached to the actual panel created from this snapshot. A queued rebuild
/// cannot make an older panel appear current before its commands are applied.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagUiTreeBuildStamp {
    pub identity: BagIdentity,
    pub model_revision: u64,
    pub presentation_revision: u64,
    pub page: BagPage,
    pub visible_items: usize,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
struct BagDetailState {
    cell: BagPaintCell,
    stamp: BagUiTreeBuildStamp,
    document: CrystalItemTooltipDocument,
    page: usize,
}

impl BagUiTreeBuildStamp {
    pub fn matches(self, context: &BagUiHostContext, model: &BagUiReadModel, page: BagPage) -> bool {
        self.identity == context.identity
            && self.model_revision == context.model_revision
            && self.presentation_revision == context.presentation_revision
            && self.page == page
            && self.visible_items == visible_bag_item_count(&model.inventory, page)
    }
}

#[derive(Debug, Default)]
pub struct BagTreeObservation {
    pub stamp: Option<BagUiTreeBuildStamp>,
    pub model_visible_items: usize,
    pub stamp_matches_model: bool,
    pub complete: bool,
    pub content_complete: bool,
    pub touch_actions: Option<BagTouchActionObservation>,
    pub grid_cells: usize,
    pub item_cells: usize,
    pub occupied_cells: usize,
    pub icon_nodes: usize,
    pub visible_icons: usize,
    pub laid_out_icons: usize,
    pub stack_text_nodes: usize,
    pub visible_stack_texts: usize,
    pub laid_out_stack_texts: usize,
}

/// Status-only geometry evidence. No item identity or item name is exported.
#[derive(Debug, Clone, Copy)]
pub struct BagTouchActionFailure {
    pub action_type: &'static str,
    pub slot: Option<u32>,
    pub reason: &'static str,
    /// [left, top, width, height] in actual stage CSS pixels.
    pub css_rect: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Default)]
pub struct BagTouchActionObservation {
    pub complete: bool,
    pub scoped_action_count: usize,
    pub expected_action_count: usize,
    pub min_css_hit_width: Option<f32>,
    pub min_css_hit_height: Option<f32>,
    pub first_failure: Option<BagTouchActionFailure>,
}

impl BagTouchActionObservation {
    fn fail(&mut self, action_type: &'static str, slot: Option<u32>,
        reason: &'static str, css_rect: Option<[f32; 4]>) {
        if self.first_failure.is_none() {
            self.first_failure = Some(BagTouchActionFailure { action_type, slot, reason, css_rect });
        }
    }
}

#[derive(SystemParam)]
pub struct BagTreeObserver<'w, 's> {
    panels: Query<'w, 's, (Entity, &'static ChildOf, &'static BagUiTreeBuildStamp), With<BagUiPanel>>,
    inspectors: Query<'w, 's, (Entity, &'static ChildOf), With<BagUiInspector>>,
    details: Query<'w, 's, (Entity, &'static ChildOf, &'static BagDetailState,
        &'static BagUiTreeBuildStamp), With<BagUiDetail>>,
    detail_bodies: Query<'w, 's, (Entity, &'static ChildOf, &'static Node,
        &'static ComputedNode, &'static bevy::ui::UiGlobalTransform,
        &'static InheritedVisibility), With<BagUiDetailBody>>,
    detail_lines: Query<'w, 's, (&'static Text, &'static ComputedNode,
        &'static bevy::ui::UiGlobalTransform, &'static InheritedVisibility),
        With<CrystalItemHintOverlayText>>,
    grids: Query<'w, 's, (Entity, &'static ChildOf), With<BagPaintGridViewport>>,
    cells: Query<'w, 's, (Entity, &'static BagPaintCell, &'static ChildOf, Option<&'static CrystalItemHint>)>,
    children: Query<'w, 's, &'static Children>,
    icons: Query<'w, 's, (&'static Node, &'static ImageNode, &'static InheritedVisibility, Option<&'static ComputedNode>), With<OriginalItemImage>>,
    texts: Query<'w, 's, (&'static Text, &'static InheritedVisibility, Option<&'static ComputedNode>)>,
    nodes: Query<'w, 's, (&'static Node, &'static InheritedVisibility, Option<&'static ComputedNode>)>,
    actions: Query<'w, 's, (Entity, &'static Node, &'static ComputedNode,
        &'static bevy::ui::UiGlobalTransform, &'static InheritedVisibility),
        Or<(With<BagPaintAction>, With<BagUiAction>)>>,
    action_markers: Query<'w, 's, Entity, Or<(With<BagPaintAction>, With<BagUiAction>)>>,
    inspector_actions: Query<'w, 's, &'static BagUiAction>,
    paint_actions: Query<'w, 's, &'static BagPaintAction>,
    parents: Query<'w, 's, &'static ChildOf>,
}

fn laid_out(computed: Option<&ComputedNode>) -> bool {
    computed.is_some_and(|node| {
        let size = node.size();
        size.is_finite() && size.x > 0.0 && size.y > 0.0
    })
}

impl BagTreeObserver<'_, '_> {
    fn detail_content_ready(&self, detail_entity: Entity, detail: &BagDetailState) -> bool {
        let Some(page) = detail_pages(&detail.document).get(detail.page).cloned() else { return false; };
        let bodies: Vec<_> = self.detail_bodies.iter()
            .filter(|(_, parent, ..)| parent.parent() == detail_entity).collect();
        let [(body, _, node, computed, transform, visibility)] = bodies.as_slice() else { return false; };
        if node.display == Display::None || !visibility.get() || !laid_out(Some(computed)) {
            return false;
        }
        let bounds = logical_node_rect(computed, transform);
        let expected: Vec<_> = page.sections.iter().flat_map(|section| &section.lines)
            .map(|line| line.text.as_str()).collect();
        if expected.is_empty() { return false; }
        let mut actual = Vec::new();
        for section in self.children.get(*body).into_iter().flat_map(|children| children.iter()) {
            for entity in self.children.get(section).into_iter().flat_map(|children| children.iter()) {
                let Ok((text, computed, transform, visible)) = self.detail_lines.get(entity) else { continue; };
                let rect = logical_node_rect(computed, transform);
                if !visible.get() || !laid_out(Some(computed))
                    || rect.min.x < bounds.min.x - 0.1 || rect.min.y < bounds.min.y - 0.1
                    || rect.max.x > bounds.max.x + 0.1 || rect.max.y > bounds.max.y + 0.1 {
                    return false;
                }
                actual.push(text.0.as_str());
            }
        }
        actual == expected
    }

    fn touch_actions_ready(&self, root: Entity, panel: Entity, expected_cells: usize,
        presentation: BagUiPresentation, state: &BagUiState,
        detail: Option<Entity>) -> BagTouchActionObservation {
        let mut result = BagTouchActionObservation {
            expected_action_count: if detail.is_some() { 3 } else {
                expected_cells + 5 + usize::from(state.selected.is_some() || state.selected_quest_slot.is_some())
            }, ..Default::default()
        };
        let inspectors: Vec<_> = self.inspectors.iter()
            .filter(|(_, parent)| parent.parent() == root).collect();
        let [(inspector, _)] = inspectors.as_slice() else {
            result.fail("surface", None, "inspector_missing", None);
            return result;
        };
        let mut rects: Vec<bevy::math::Rect> = Vec::new();
        let mut more = false;
        let mut inspect = false;
        let mut detail_buttons = [false; 3];
        for entity in &self.action_markers {
            let mut ancestor = entity;
            let mut scoped = false;
            while let Ok(parent) = self.parents.get(ancestor) {
                ancestor = parent.parent();
                if detail.is_some_and(|detail| ancestor == detail) {
                    scoped = true; break;
                }
                if detail.is_none() && (ancestor == panel || ancestor == *inspector) {
                    scoped = true; break;
                }
                if ancestor == root { break; }
            }
            if !scoped { continue; }
            result.scoped_action_count += 1;
            let (action_type, slot) = match self.paint_actions.get(entity) {
                Ok(BagPaintAction::InspectCell { slot, .. }) => ("cell", Some(*slot)),
                Ok(BagPaintAction::SelectPage(_)) => ("page", None),
                Ok(BagPaintAction::Close) => ("close", None),
                Ok(BagPaintAction::ToggleDelete) => ("delete", None),
                Err(_) => match self.inspector_actions.get(entity) {
                    Ok(BagUiAction::Use) => ("use", None),
                    Ok(BagUiAction::Equip) => ("equip", None),
                    Ok(BagUiAction::Move) => ("move", None),
                    Ok(BagUiAction::More) => ("more", None),
                    Ok(BagUiAction::Inspect) => { inspect = true; ("inspect", None) },
                    Ok(BagUiAction::DetailBack) => { detail_buttons[0] = true; ("detail_back", None) },
                    Ok(BagUiAction::DetailPrev) => { detail_buttons[1] = true; ("detail_prev", None) },
                    Ok(BagUiAction::DetailNext) => { detail_buttons[2] = true; ("detail_next", None) },
                    _ => ("action", None),
                },
            };
            let Ok((_, node, computed, transform, visible)) = self.actions.get(entity) else {
                result.fail(action_type, slot, "layout_missing", None);
                continue;
            };
            let rect = logical_node_rect(computed, transform);
            let size = rect.size();
            let css_rect = [rect.min.x * presentation.stage_css_scale,
                rect.min.y * presentation.stage_css_scale,
                size.x * presentation.stage_css_scale,
                size.y * presentation.stage_css_scale];
            let css_rect_opt = css_rect.iter().all(|value| value.is_finite()).then_some(css_rect);
            if css_rect_opt.is_some() {
                result.min_css_hit_width = Some(result.min_css_hit_width
                    .map_or(css_rect[2], |min| min.min(css_rect[2])));
                result.min_css_hit_height = Some(result.min_css_hit_height
                    .map_or(css_rect[3], |min| min.min(css_rect[3])));
            }
            if node.display == Display::None || !visible.get() {
                result.fail(action_type, slot, "hidden", css_rect_opt);
                continue;
            }
            if css_rect_opt.is_none() {
                result.fail(action_type, slot, "invalid_geometry", None);
                continue;
            }
            if css_rect[2] < 40.0 || css_rect[3] < 40.0 {
                result.fail(action_type, slot, "hit_below_40_css", css_rect_opt);
                continue;
            }
            if rect.min.x < -0.1 || rect.min.y < -0.1
                || rect.max.x > presentation.logical_width + 0.1
                || rect.max.y > presentation.logical_height + 0.1 {
                result.fail(action_type, slot, "outside_stage", css_rect_opt);
                continue;
            }
            if rects.iter().any(|other| rect.min.x < other.max.x - 0.1
                && rect.max.x > other.min.x + 0.1
                && rect.min.y < other.max.y - 0.1
                && rect.max.y > other.min.y + 0.1) {
                result.fail(action_type, slot, "overlap", css_rect_opt);
                continue;
            }
            rects.push(rect);
        }
        if detail.is_some() {
            if !detail_buttons.into_iter().all(|present| present) {
                result.fail("detail", None, "navigation_missing", None);
            }
        } else {
            for child in self.children.get(*inspector).into_iter().flat_map(|children| children.iter()) {
                if self.inspector_actions.get(child).is_ok_and(|action| *action == BagUiAction::More)
                    && self.actions.get(child).is_ok() { more = true; }
            }
            if !more { result.fail("more", None, "more_missing", None); }
            if (state.selected.is_some() || state.selected_quest_slot.is_some()) && !inspect {
                result.fail("inspect", None, "inspect_missing", None);
            }
        }
        if result.scoped_action_count < result.expected_action_count {
            result.fail("surface", None, "action_count_short", None);
        }
        result.complete = result.first_failure.is_none();
        result
    }

    pub fn observe(&self, root: Entity, context: &BagUiHostContext, model: &BagUiReadModel,
        page: BagPage, state: &BagUiState) -> BagTreeObservation {
        let mut result = BagTreeObservation {
            model_visible_items: visible_bag_item_count(&model.inventory, page),
            ..Default::default()
        };
        let panels: Vec<_> = self.panels.iter().filter(|(_, parent, _)| parent.parent() == root).collect();
        let [(panel, _, stamp)] = panels.as_slice() else { return result; };
        result.stamp = Some(**stamp);
        result.stamp_matches_model = stamp.matches(context, model, page);
        let grids: Vec<_> = self.grids.iter().filter(|(_, parent)| parent.parent() == *panel).collect();
        let [(grid, _)] = grids.as_slice() else { return result; };
        let (container, first_slot, capacity) = match page {
            BagPage::Bag1 => (0, 0, u32::from(model.inventory.bag_slot_capacity())),
            BagPage::Bag2 => (0, 40, u32::from(model.inventory.bag_slot_capacity())),
            BagPage::Quest => (3, 0, 40),
        };
        let expected_slots = (first_slot..first_slot + 40).filter(|slot| *slot < capacity).count();
        let mut seen = HashSet::new();
        let mut valid = result.stamp_matches_model
            && self.nodes.get(root).is_ok_and(|(node, visibility, _)| node.display != Display::None && visibility.get())
            && self.nodes.get(*grid).is_ok_and(|(node, visibility, computed)|
                node.display != Display::None && visibility.get() && laid_out(computed));
        for (entity, cell, parent, hint) in &self.cells {
            if parent.parent() != *grid { continue; }
            result.grid_cells += 1;
            if !self.nodes.get(entity).is_ok_and(|(node, visibility, computed)|
                node.display != Display::None && visibility.get() && laid_out(computed)) { valid = false; }
            if cell.unique_id.is_some() { result.item_cells += 1; }
            if hint.is_some() { result.occupied_cells += 1; }
            if cell.container != container || cell.slot < first_slot || cell.slot >= first_slot + 40
                || cell.slot >= capacity || !seen.insert(cell.slot) { valid = false; continue; }
            let expected = model.inventory.items.iter().find(|item| item.container == container && item.slot == cell.slot);
            let Some(item) = expected else {
                if hint.is_some() || cell.unique_id.is_some() { valid = false; }
                continue;
            };
            if hint.is_none() || cell.unique_id != item.unique_id { valid = false; }
            let mut icon_count = 0;
            let mut ready_icon_count = 0;
            let mut text_count = 0;
            let mut ready_text_count = 0;
            if let Ok(cell_children) = self.children.get(entity) {
                for child in cell_children.iter() {
                    if let Ok((node, image, visibility, computed)) = self.icons.get(child) {
                        icon_count += 1;
                        result.icon_nodes += 1;
                        let expected_path = item.user_item_image_index()
                            .map(|index| format!("original-ui/Items/{index}.png"));
                        let correct_image = image.image.path().is_some_and(|path|
                            expected_path.as_deref() == Some(path.to_string().as_str()));
                        if node.display != Display::None && visibility.get() && correct_image {
                            result.visible_icons += 1;
                            if laid_out(computed) { result.laid_out_icons += 1; ready_icon_count += 1; }
                        }
                    }
                    let Ok(label_children) = self.children.get(child) else { continue; };
                    let wrapper_ready = self.nodes.get(child).is_ok_and(|(node, visibility, computed)|
                        node.display != Display::None && visibility.get() && laid_out(computed));
                    for label in label_children.iter() {
                        if let Ok((text, visibility, computed)) = self.texts.get(label) {
                            text_count += 1;
                            result.stack_text_nodes += 1;
                            if wrapper_ready && visibility.get() {
                                result.visible_stack_texts += 1;
                                if laid_out(computed) {
                                    result.laid_out_stack_texts += 1;
                                    if text.0 == item.crystal_stack_label() { ready_text_count += 1; }
                                }
                            }
                        }
                    }
                }
            }
            if item.user_item_image_index().is_none() || icon_count != 1 || ready_icon_count != 1 { valid = false; }
            let expected_stack = !item.crystal_stack_label().is_empty();
            if text_count != usize::from(expected_stack) || ready_text_count != usize::from(expected_stack) { valid = false; }
        }
        result.content_complete = valid && result.grid_cells == expected_slots && seen.len() == expected_slots
            && result.occupied_cells == result.model_visible_items;
        let detail = if state.detail.is_some() {
            let candidates: Vec<_> = self.details.iter().filter(|(_, parent, _, _)| parent.parent() == root).collect();
            match (state.detail.as_ref(), candidates.as_slice()) {
                (Some(expected), [(entity, _, actual, actual_stamp)])
                    if *actual == expected && *actual_stamp == *stamp
                        && selected_paint_cell(state, model) == Some(expected.cell)
                        && self.detail_content_ready(*entity, expected)
                        && self.cells.iter().any(|(_, cell, parent, hint)| {
                            *cell == expected.cell && hint.is_some_and(|hint| hint.0 == expected.document)
                                && self.grids.get(parent.parent()).is_ok_and(|(_, grid_parent)| grid_parent.parent() == *panel)
                        }) => Some(*entity),
                _ => { valid = false; None }
            }
        } else {
            if self.details.iter().any(|(_, parent, _, _)| parent.parent() == root) { valid = false; }
            None
        };
        result.content_complete &= valid;
        result.touch_actions = context.presentation.filter(|presentation| presentation.touch)
            .map(|presentation| self.touch_actions_ready(root, *panel,
                if page == BagPage::Quest { result.model_visible_items } else { expected_slots },
                presentation, state, detail));
        result.complete = result.content_complete
            && result.touch_actions.as_ref().is_none_or(|actions| actions.complete);
        result
    }
}

pub fn visible_bag_item_count(inventory: &InventoryModel, page: BagPage) -> usize {
    inventory.items.iter().filter(|item| match page {
        BagPage::Bag1 => item.container == 0 && item.slot < 40,
        BagPage::Bag2 => item.container == 0 && (40..80).contains(&item.slot),
        BagPage::Quest => item.container == 3 && item.slot < 40,
    }).count()
}

// A bounded number of short, source-coloured lines keeps every page inside the
// small landscape detail panel. Splitting only presentation text preserves the
// original document, including its honest source_complete flag.
pub(crate) fn detail_pages(document: &CrystalItemTooltipDocument) -> Vec<CrystalItemTooltipDocument> {
    const UNITS_PER_LINE: usize = 48;
    const LINES_PER_PAGE: usize = 5;
    let mut pages = Vec::new();
    let mut sections: Vec<CrystalItemTooltipSection> = Vec::new();
    let mut line_count = 0;
    for section in &document.sections {
        for line in &section.lines {
            let mut chunks = Vec::new();
            let mut chunk = String::new();
            let mut units = 0;
            for ch in line.text.chars() {
                let width = if ch.is_ascii() { 1 } else { 2 };
                if units + width > UNITS_PER_LINE && !chunk.is_empty() {
                    chunks.push(std::mem::take(&mut chunk));
                    units = 0;
                }
                chunk.push(ch);
                units += width;
            }
            chunks.push(chunk);
            for chunk in chunks {
                if line_count == LINES_PER_PAGE {
                    pages.push(CrystalItemTooltipDocument {
                        sections: std::mem::take(&mut sections),
                        broken: document.broken,
                        source_complete: document.source_complete,
                    });
                    line_count = 0;
                }
                if sections.last().is_none_or(|last| last.kind != section.kind) || line_count == 0 {
                    sections.push(CrystalItemTooltipSection { kind: section.kind, lines: Vec::new() });
                }
                sections.last_mut().unwrap().lines.push(CrystalItemTooltipLine {
                    text: chunk,
                    colour: line.colour,
                });
                line_count += 1;
            }
        }
    }
    pages.push(CrystalItemTooltipDocument {
        sections, broken: document.broken, source_complete: document.source_complete,
    });
    pages
}

fn selected_paint_cell(state: &BagUiState, model: &BagUiReadModel) -> Option<BagPaintCell> {
    match state.page {
        BagPage::Quest => {
            let slot = state.selected_quest_slot?;
            let item = model.inventory.items.iter().find(|item| item.container == 3 && item.slot == slot)?;
            Some(BagPaintCell { container: 3, slot, unique_id: item.unique_id })
        }
        BagPage::Bag1 | BagPage::Bag2 => {
            let selected = state.selected?;
            let in_page = match state.page {
                BagPage::Bag1 => selected.slot < 40,
                BagPage::Bag2 => (40..80).contains(&selected.slot),
                BagPage::Quest => false,
            };
            (selected.container == 0 && in_page && resolve_exact(&model.inventory, selected).is_some()).then_some(BagPaintCell {
                container: selected.container, slot: selected.slot, unique_id: Some(selected.unique_id),
            })
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagUiAction {
    Paint(BagPaintAction),
    Use,
    Equip,
    Move,
    More,
    Inspect,
    DetailBack,
    DetailPrev,
    DetailNext,
}

#[derive(Debug, Clone, Copy)]
struct PointerLease {
    pointer_id: u64,
    identity: BagIdentity,
    presentation_revision: u64,
    origin: BagPointerOrigin,
    start: Option<BagUiAction>,
}

#[derive(Resource, Default)]
struct PointerState {
    lease: Option<PointerLease>,
    quarantined: HashSet<u64>,
    last_sequence: u64,
}

pub struct Mir2PortableBagUiPlugin;

impl Plugin for Mir2PortableBagUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BagUiHostContext>()
            .init_resource::<BagUiReadModel>()
            .init_resource::<BagUiState>()
            .init_resource::<BagUiPointerEdges>()
            .init_resource::<BagUiIntentQueue>()
            .init_resource::<PointerState>()
            .add_systems(Startup, spawn_bag_root)
            .add_systems(
                Update,
                (apply_bag_context, process_bag_pointer_edges, render_bag_ui,
                    layout_original_item_images)
                    .chain().after(PendingLifecycleSet::Ingest),
            );
    }
}

fn spawn_bag_root(
    mut commands: Commands,
    target_camera: Option<Res<QuestUiTargetCamera>>,
) {
    let Some(camera) = target_camera else { return; };
    commands.spawn((
        BagUiRoot,
        UiTargetCamera(camera.0),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        FocusPolicy::Pass,
        GlobalZIndex(990),
    ));
}

fn apply_bag_context(
    context: Res<BagUiHostContext>,
    model: Res<BagUiReadModel>,
    mut state: ResMut<BagUiState>,
    mut pointer: ResMut<PointerState>,
    mut queue: ResMut<BagUiIntentQueue>,
) {
    let old_identity = state.last_identity;
    let old_presentation = pointer.lease.map(|lease| lease.presentation_revision);
    let had_detail = state.detail.is_some();
    let changed = state.apply_context(&context, &model);
    if model.is_changed() { state.detail = None; }
    if changed || old_presentation.is_some_and(|revision| revision != context.presentation_revision)
        || (had_detail && state.detail.is_none())
        || !context.input_enabled || !context.bag_open || !context.presentation_ready
    {
        if let Some(lease) = pointer.lease.take() {
            pointer.quarantined.insert(lease.pointer_id);
        }
    }
    if old_identity != Some(context.identity) {
        pointer.last_sequence = 0;
        queue.clear();
    }
}

fn action_at(
    point: Vec2,
    actions: &Query<(Entity, &BagUiAction, &ComputedNode, &bevy::ui::UiGlobalTransform, &InheritedVisibility)>,
    parents: &Query<&ChildOf>,
    detail: Option<Entity>,
    detail_active: bool,
) -> Option<BagUiAction> {
    actions.iter().find_map(|(entity, action, node, transform, visibility)| {
        if detail_active {
            let Some(detail) = detail else { return None; };
            let mut ancestor = entity;
            let mut inside = false;
            while let Ok(parent) = parents.get(ancestor) {
                ancestor = parent.parent();
                if ancestor == detail { inside = true; break; }
            }
            if !inside { return None; }
        } else if matches!(action, BagUiAction::DetailBack | BagUiAction::DetailPrev | BagUiAction::DetailNext) {
            return None;
        }
        if !visibility.get() { return None; }
        logical_node_rect(node, transform).contains(point).then_some(*action)
    })
}

fn painted_detail(
    context: &BagUiHostContext,
    model: &BagUiReadModel,
    page: BagPage,
    cell: BagPaintCell,
    roots: &Query<Entity, With<BagUiRoot>>,
    panels: &Query<(&BagUiTreeBuildStamp, &ChildOf), With<BagUiPanel>>,
    grids: &Query<&ChildOf, With<BagPaintGridViewport>>,
    cells: &Query<(&BagPaintCell, &CrystalItemHint, &ChildOf)>,
) -> Option<BagDetailState> {
    let root = roots.single().ok()?;
    for (painted, hint, parent) in cells.iter() {
        if *painted != cell { continue; }
        let Ok(grid_parent) = grids.get(parent.parent()) else { continue; };
        let Ok((stamp, panel_parent)) = panels.get(grid_parent.parent()) else { continue; };
        if panel_parent.parent() == root && stamp.matches(context, model, page)
            && hint.0.sections.iter().any(|section| !section.lines.is_empty()) {
            return Some(BagDetailState {
                cell, stamp: *stamp, document: hint.0.clone(), page: 0,
            });
        }
    }
    None
}

fn logical_node_rect(node: &ComputedNode, transform: &bevy::ui::UiGlobalTransform) -> bevy::math::Rect {
    let scale = node.inverse_scale_factor;
    let center = transform.affine().translation * scale;
    let half = node.size() * scale * 0.5;
    bevy::math::Rect { min: center - half, max: center + half }
}

fn process_bag_pointer_edges(
    context: Res<BagUiHostContext>,
    model: Res<BagUiReadModel>,
    mut state: ResMut<BagUiState>,
    mut pointer: ResMut<PointerState>,
    mut edges: ResMut<BagUiPointerEdges>,
    mut queue: ResMut<BagUiIntentQueue>,
    actions: Query<(Entity, &BagUiAction, &ComputedNode, &bevy::ui::UiGlobalTransform, &InheritedVisibility)>,
    parents: Query<&ChildOf>,
    details: Query<(Entity, &BagDetailState), With<BagUiDetail>>,
    roots: Query<Entity, With<BagUiRoot>>,
    panels: Query<(&BagUiTreeBuildStamp, &ChildOf), With<BagUiPanel>>,
    grids: Query<&ChildOf, With<BagPaintGridViewport>>,
    cells: Query<(&BagPaintCell, &CrystalItemHint, &ChildOf)>,
) {
    for edge in std::mem::take(&mut edges.0) {
        if edge.identity != context.identity || edge.presentation_revision != context.presentation_revision {
            // Old-owner termination is cleanup only; it must not poison the
            // new epoch's sequence or become a click in its geometry.
            if matches!(edge.phase, BagPointerPhase::Up | BagPointerPhase::Cancel | BagPointerPhase::Blur) {
                pointer.quarantined.remove(&edge.pointer_id);
                if pointer.lease.is_some_and(|lease| lease.pointer_id == edge.pointer_id
                    && lease.identity == edge.identity
                    && lease.presentation_revision == edge.presentation_revision) {
                    pointer.lease = None;
                }
            }
            continue;
        }
        if edge.sequence <= pointer.last_sequence { continue; }
        pointer.last_sequence = edge.sequence;
        if edge.phase == BagPointerPhase::Blur {
            if let Some(lease) = pointer.lease.take() { pointer.quarantined.insert(lease.pointer_id); }
            state.detail = None;
            continue;
        }
        let current = context.input_enabled && context.bag_open && context.presentation_ready
            && edge.identity == context.identity
            && edge.presentation_revision == context.presentation_revision;
        if !current && matches!(edge.phase, BagPointerPhase::Up | BagPointerPhase::Cancel) {
            pointer.quarantined.remove(&edge.pointer_id);
            if pointer.lease.is_some_and(|lease| lease.pointer_id == edge.pointer_id) {
                pointer.lease = None;
            }
            continue;
        }
        if pointer.quarantined.contains(&edge.pointer_id) {
            if matches!(edge.phase, BagPointerPhase::Up | BagPointerPhase::Cancel) {
                pointer.quarantined.remove(&edge.pointer_id);
                continue;
            }
            // A browser pointerId can be reused after its old owner was
            // invalidated. A fresh down in the *current* epoch proves that
            // the old physical gesture ended, even if its up was discarded.
            if edge.phase != BagPointerPhase::Down || !current { continue; }
            pointer.quarantined.remove(&edge.pointer_id);
        }
        if !current {
            if edge.phase == BagPointerPhase::Down { pointer.quarantined.insert(edge.pointer_id); }
            continue;
        }
        match edge.phase {
            BagPointerPhase::Down => {
                if pointer.lease.is_some() {
                    pointer.quarantined.insert(edge.pointer_id);
                    continue;
                }
                pointer.lease = Some(PointerLease {
                    pointer_id: edge.pointer_id,
                    identity: edge.identity,
                    presentation_revision: edge.presentation_revision,
                    origin: edge.origin,
                    start: (edge.origin == BagPointerOrigin::Bag && edge.button == 0)
                        .then(|| action_at(Vec2::new(edge.x, edge.y), &actions, &parents,
                            details.iter().find(|(_, detail)| state.detail.as_ref() == Some(*detail))
                                .map(|(entity, _)| entity), state.detail.is_some())).flatten(),
                });
            }
            BagPointerPhase::Move => {}
            BagPointerPhase::Cancel => {
                if pointer.lease.is_some_and(|lease| lease.pointer_id == edge.pointer_id) {
                    pointer.lease = None;
                }
            }
            BagPointerPhase::Up => {
                if !pointer.lease.is_some_and(|lease| lease.pointer_id == edge.pointer_id) { continue; }
                let Some(lease) = pointer.lease.take() else { continue; };
                if lease.origin != BagPointerOrigin::Bag || lease.identity != context.identity
                    || lease.presentation_revision != context.presentation_revision || edge.button != 0
                { continue; }
                let end = action_at(Vec2::new(edge.x, edge.y), &actions, &parents,
                    details.iter().find(|(_, detail)| state.detail.as_ref() == Some(*detail))
                        .map(|(entity, _)| entity), state.detail.is_some());
                let (Some(start), Some(end)) = (lease.start, end) else { continue; };
                if let (BagUiAction::Paint(BagPaintAction::InspectCell {container:0, slot:from, unique_id:Some(uid)}),
                    BagUiAction::Paint(BagPaintAction::InspectCell {container:0, slot:to, ..})) = (start, end)
                {
                    if from != to {
                        let source = BagSelection { container: 0, slot: from, unique_id: uid };
                        if resolve_exact(&model.inventory, source).is_some()
                            && !model.blocked_unique_ids.contains(&uid)
                            && to < u32::from(model.inventory.bag_slot_capacity())
                        {
                            let _ = queue.push(&context, BagUiIntentKind::MoveItem { source, target_slot: to });
                        }
                        state.move_source = None;
                        continue;
                    }
                }
                if start == end {
                    if start == BagUiAction::Inspect {
                        state.detail = selected_paint_cell(&state, &model).and_then(|cell|
                            painted_detail(&context, &model, state.page, cell,
                                &roots, &panels, &grids, &cells));
                    } else {
                        activate_action(start, &context, &model, &mut state, &mut queue);
                    }
                }
            }
            BagPointerPhase::Blur => unreachable!(),
        }
    }
}

fn activate_action(
    action: BagUiAction,
    context: &BagUiHostContext,
    model: &BagUiReadModel,
    state: &mut BagUiState,
    queue: &mut BagUiIntentQueue,
) {
    match action {
        BagUiAction::Paint(BagPaintAction::SelectPage(index)) => {
            if let Some(page) = BagPage::from_paint_index(index).filter(|page| *page != BagPage::Bag2 || model.inventory.second_bag_unlocked()) {
                state.detail = None;
                state.selected = None;
                state.selected_quest_slot = None;
                state.move_source = None;
                let _ = queue.push(context, BagUiIntentKind::SelectPage(page));
            }
        }
        BagUiAction::Paint(BagPaintAction::Close) => {
            state.detail = None;
            let _ = queue.push(context, BagUiIntentKind::Close);
        }
        BagUiAction::Paint(BagPaintAction::ToggleDelete) => { let _ = queue.push(context, BagUiIntentKind::HandoffDelete); }
        BagUiAction::Paint(BagPaintAction::InspectCell { container: 3, slot, .. }) => {
            state.selected = None;
            state.move_source = None;
            state.selected_quest_slot = Some(slot);
        }
        BagUiAction::Paint(BagPaintAction::InspectCell { container: 0, slot, unique_id }) => {
            if let Some(source) = state.move_source.take() {
                if source.slot != slot && resolve_exact(&model.inventory, source).is_some()
                    && !model.blocked_unique_ids.contains(&source.unique_id)
                    && slot < u32::from(model.inventory.bag_slot_capacity())
                {
                    let _ = queue.push(context, BagUiIntentKind::MoveItem { source, target_slot: slot });
                    return;
                }
            }
            state.selected = unique_id.and_then(|unique_id| {
                let selection = BagSelection { container: 0, slot, unique_id };
                resolve_exact(&model.inventory, selection).map(|_| selection)
            });
            state.selected_quest_slot = None;
        }
        BagUiAction::Paint(BagPaintAction::InspectCell { .. }) => {}
        BagUiAction::Use | BagUiAction::Equip | BagUiAction::Move => {
            let Some(source) = state.selected.filter(|source| {
                resolve_exact(&model.inventory, *source).is_some()
                    && !model.blocked_unique_ids.contains(&source.unique_id)
            }) else { return; };
            match action {
                BagUiAction::Use => { let _ = queue.push(context, BagUiIntentKind::UseItem { source }); }
                BagUiAction::Equip => {
                    if resolve_exact(&model.inventory, source).is_some_and(|item| item.equip_slot.is_some()) {
                        let _ = queue.push(context, BagUiIntentKind::EquipItem { source });
                    }
                }
                BagUiAction::Move => state.move_source = Some(source),
                _ => unreachable!(),
            }
        }
        BagUiAction::More => {
            state.detail = None;
            let _ = queue.push(context, BagUiIntentKind::HandoffFullInventory);
        }
        BagUiAction::DetailBack => state.detail = None,
        BagUiAction::DetailPrev => {
            if let Some(detail) = &mut state.detail { detail.page = detail.page.saturating_sub(1); }
        }
        BagUiAction::DetailNext => {
            if let Some(detail) = &mut state.detail {
                let count = detail_pages(&detail.document).len();
                detail.page = (detail.page + 1).min(count.saturating_sub(1));
            }
        }
        BagUiAction::Inspect => {}
    }
}

fn action_button(parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands, label: &str, action: BagUiAction, font: &TextFont, layout: BagPaintLayout) {
    parent.spawn((
        action,
        Button,
        Node {
            width: Val::Px(106.0 * layout.scale),
            height: Val::Px(if layout.touch { 32.0 } else { 28.0 } * layout.scale),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(4.0 * layout.scale)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.21, 0.17, 0.12, 0.95)),
    )).with_children(|button| {
        button.spawn((Text::new(label), font.clone(), TextColor(Color::WHITE)));
    });
}

fn inspector_name(parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands,
    name: String, font: &TextFont, layout: BagPaintLayout) {
    let mut entity = parent.spawn((Text::new(name), font.clone(), TextColor(Color::WHITE)));
    if layout.touch {
        // The rich detail contains the full name; keep long labels from
        // pushing the touch actions below the bottom of the viewport.
        entity.insert((
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            Node { width: Val::Percent(100.0), height: Val::Px(18.0 * layout.scale),
                overflow: Overflow::clip(), ..default() },
        ));
    }
}

fn detail_button(parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands,
    label: &str, action: BagUiAction, left: f32, layout: BagPaintLayout, font: &TextFont) {
    parent.spawn((
        action,
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left * layout.scale),
            top: Val::Px(196.0 * layout.scale),
            width: Val::Px(106.0 * layout.scale),
            height: Val::Px(32.0 * layout.scale),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.21, 0.17, 0.12, 1.0)),
    )).with_children(|button| {
        button.spawn((Text::new(label), font.clone(), TextColor(Color::WHITE)));
    });
}

fn render_bag_ui(
    mut commands: Commands,
    roots: Query<Entity, With<BagUiRoot>>,
    mut panels: Query<&mut Node, With<BagUiRoot>>,
    context: Res<BagUiHostContext>,
    model: Res<BagUiReadModel>,
    state: Res<BagUiState>,
    font: Option<Res<QuestUiFont>>,
    assets: Option<Res<AssetServer>>,
    mut last: Local<Option<(BagIdentity, u64, BagPage, Option<BagSelection>, Option<u32>, Option<BagSelection>, bool, Option<BagUiPresentation>, Option<(BagPaintCell, usize)>)>>,
) {
    let Ok(root) = roots.single() else { return; };
    let Ok(mut node) = panels.get_mut(root) else { return; };
    let visible = context.bag_open && context.presentation_ready;
    let display = if visible { Display::Flex } else { Display::None };
    if node.display != display { node.display = display; }
    if !visible { return; }
    let key = (context.identity, context.model_revision, state.page, state.selected,
        state.selected_quest_slot, state.move_source, context.input_enabled, context.presentation,
        state.detail.as_ref().map(|detail| (detail.cell, detail.page)));
    if last.as_ref() == Some(&key) && !model.is_changed() && !font.as_ref().is_some_and(|font| font.is_changed()) {
        return;
    }
    let (Some(font), Some(assets)) = (font, assets) else { return; };
    *last = Some(key);
    let stamp = BagUiTreeBuildStamp {
        identity: context.identity,
        model_revision: context.model_revision,
        presentation_revision: context.presentation_revision,
        page: state.page,
        visible_items: visible_bag_item_count(&model.inventory, state.page),
    };
    let presentation = context.presentation;
    let layout = presentation.map_or(BagPaintLayout::default(), BagUiPresentation::paint_layout);
    let (panel_left, panel_top) = if layout.touch {
        let metrics = presentation.expect("touch layout has presentation");
        ((metrics.logical_width - 448.0 * layout.scale) * 0.5,
            (metrics.logical_height - 236.0 * layout.scale) * 0.5)
    } else { (410.0, 86.0) };
    let text_font = TextFont { font: FontSource::Handle(font.0.clone()),
        font_size: FontSize::Px(13.0 * layout.scale), ..default() };
    commands.entity(root).despawn_children();
    commands.entity(root).with_children(|children| {
        children.spawn((
            BagUiPanel,
            stamp,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_left),
                top: Val::Px(panel_top),
                width: Val::Px(316.0 * layout.scale),
                height: Val::Px(236.0 * layout.scale),
                ..default()
            },
            BackgroundColor(Color::NONE),
        )).with_children(|panel| {
            let options = BagPaintOptions {
                page: state.page.paint_index(),
                delete_mode: false,
                weight_ratio: (model.player.max_weight > 0 && model.player.current_weight_known)
                    .then(|| model.player.normalized_weight()),
                free_slots: Some(u32::from(model.inventory.bag_slot_capacity()).saturating_sub(
                    model.inventory.items.iter().filter(|item| item.container == 0).count() as u32,
                )),
                font: text_font.clone(),
                stack_split_hint: crate::crystal_ui::item_tooltip::CrystalStackSplitHint::MoreActions,
            };
            paint_crystal_inventory_with_layout(panel, &assets, &model.inventory, &model.player, &options, layout,
                BagUiAction::Paint,
                |cell| BagCellPolicy::new(true,
                    cell.container == 3 && cell.item.is_some() || cell.container == 0,
                    cell.item.and_then(|item| item.unique_id)
                        .is_some_and(|id| model.blocked_unique_ids.contains(&id))),
                |_, _| {});
        });
        children.spawn((
            BagUiInspector,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(if layout.touch { panel_left + 326.0 * layout.scale } else { 736.0 }),
                top: Val::Px(panel_top),
                width: Val::Px(122.0 * layout.scale),
                padding: UiRect::all(Val::Px(6.0 * layout.scale)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.05, 0.04, 0.96)),
        )).with_children(|inspect| {
            if let Some(selection) = state.selected {
                if let Some(item) = resolve_exact(&model.inventory, selection) {
                    inspector_name(inspect, item.name.clone(), &text_font, layout);
                    if layout.touch { action_button(inspect, "Inspect", BagUiAction::Inspect, &text_font, layout); }
                    action_button(inspect, "Use", BagUiAction::Use, &text_font, layout);
                    if item.equip_slot.is_some() { action_button(inspect, "Equip", BagUiAction::Equip, &text_font, layout); }
                    action_button(inspect, if state.move_source.is_some() { "Move: choose slot" } else { "Move" }, BagUiAction::Move, &text_font, layout);
                }
            } else if state.selected_quest_slot.is_some() {
                let name = state.selected_quest_slot.and_then(|slot| model.inventory.items.iter()
                    .find(|item| item.container == 3 && item.slot == slot))
                    .map_or("Quest item", |item| item.name.as_str());
                inspector_name(inspect, name.to_owned(), &text_font, layout);
                if layout.touch && selected_paint_cell(&state, &model).is_some() {
                    action_button(inspect, "Inspect", BagUiAction::Inspect, &text_font, layout);
                }
            } else {
                inspector_name(inspect, "Select an item".to_owned(), &text_font, layout);
            }
            action_button(inspect, "More actions", BagUiAction::More, &text_font, layout);
        });
        if layout.touch {
            if let Some(detail) = state.detail.as_ref() {
                let pages = detail_pages(&detail.document);
                let page = detail.page.min(pages.len() - 1);
                let css_scale = presentation.expect("touch detail has presentation").stage_css_scale;
                let detail_font = TextFont { font: FontSource::Handle(font.0.clone()),
                    font_size: FontSize::Px(14.0 / css_scale), ..default() };
                children.spawn((
                    BagUiDetail,
                    detail.clone(),
                    stamp,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(panel_left), top: Val::Px(panel_top),
                        width: Val::Px(448.0 * layout.scale),
                        height: Val::Px(236.0 * layout.scale),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.055, 0.045, 0.035, 1.0)),
                    FocusPolicy::Block,
                    GlobalZIndex(CRYSTAL_HINT_Z_INDEX + 2),
                )).with_children(|detail_root| {
                    detail_root.spawn((
                        Text::new(format!("Item details{} — {}/{}",
                            if detail.document.source_complete { "" } else { " (partial)" },
                            page + 1, pages.len())),
                        detail_font.clone(), TextColor(Color::WHITE),
                        Node { position_type: PositionType::Absolute,
                            left: Val::Px(8.0 * layout.scale), top: Val::Px(6.0 * layout.scale),
                            ..default() },
                    ));
                    detail_root.spawn((
                        BagUiDetailBody,
                        Node { position_type: PositionType::Absolute,
                            left: Val::Px(8.0 * layout.scale), top: Val::Px(36.0 * layout.scale),
                            width: Val::Px(432.0 * layout.scale),
                            height: Val::Px(150.0 * layout.scale),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::clip(), ..default() },
                    )).with_children(|body| {
                        spawn_crystal_item_hint_document(body, &pages[page], &detail_font);
                    });
                    detail_button(detail_root, "Back", BagUiAction::DetailBack, 8.0, layout, &detail_font);
                    detail_button(detail_root, "Prev", BagUiAction::DetailPrev, 171.0, layout, &detail_font);
                    detail_button(detail_root, "Next", BagUiAction::DetailNext, 334.0, layout, &detail_font);
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    use crate::crystal_ui::item_tooltip::{CrystalItemTooltipColour,
        CrystalItemTooltipSectionKind};

    struct TreeFixture {
        world: World,
        root: Entity,
        panel: Entity,
        first_cell: Entity,
        icon: Option<Entity>,
        text: Option<Entity>,
        context: BagUiHostContext,
        model: BagUiReadModel,
        page: BagPage,
    }

    fn tree_fixture(page: BagPage, item: Option<(Option<u64>, u32)>) -> TreeFixture {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>();
        let mut world = std::mem::take(app.world_mut());
        let mut context = BagUiHostContext::default();
        context.identity.owner_revision = 6;
        context.model_revision = 4;
        context.presentation_revision = 9;
        let mut model = BagUiReadModel::default();
        let (container, first_slot) = if page == BagPage::Quest { (3, 0) } else { (0, 0) };
        if let Some((unique_id, quantity)) = item {
            model.inventory.items.push(crate::inventory::ItemModel {
                unique_id, container, slot: first_slot, quantity, icon: 1,
                name: "Fixture item".into(), ..default()
            });
        }
        let root = world.spawn((BagUiRoot, Node::default(), InheritedVisibility::VISIBLE)).id();
        let panel = world.spawn((BagUiPanel, BagUiTreeBuildStamp {
            identity: context.identity, model_revision: context.model_revision,
            presentation_revision: context.presentation_revision, page,
            visible_items: usize::from(item.is_some()),
        }, ChildOf(root))).id();
        let grid = world.spawn((BagPaintGridViewport, ChildOf(panel), Node::default(),
            InheritedVisibility::VISIBLE,
            ComputedNode { size: Vec2::new(200.0, 200.0), ..default() })).id();
        let mut first_cell = None;
        let mut icon = None;
        let mut text = None;
        for slot in 0..40 {
            let occupied = slot == 0 && item.is_some();
            let mut cell = world.spawn((BagPaintCell {
                container, slot, unique_id: if occupied { item.unwrap().0 } else { None },
            }, ChildOf(grid), Node::default(), InheritedVisibility::VISIBLE,
                ComputedNode { size: Vec2::new(36.0, 32.0), ..default() }));
            if occupied {
                cell.insert(CrystalItemHint(CrystalItemTooltipDocument {
                    sections: Vec::new(), broken: false, source_complete: false,
                }));
            }
            let cell = cell.id();
            if slot == 0 { first_cell = Some(cell); }
            if occupied {
                let handle = world.resource::<AssetServer>().load::<Image>("original-ui/Items/1.png");
                icon = Some(world.spawn((OriginalItemImage { cell_width: 36, cell_height: 32 },
                    ImageNode::new(handle), Node::default(), ChildOf(cell),
                    InheritedVisibility::VISIBLE,
                    ComputedNode { size: Vec2::new(36.0, 32.0), ..default() })).id());
                if item.unwrap().1 > 1 {
                    let wrapper = world.spawn((Node::default(), ChildOf(cell), InheritedVisibility::VISIBLE,
                        ComputedNode { size: Vec2::new(36.0, 32.0), ..default() })).id();
                    text = Some(world.spawn((Text::new(item.unwrap().1.to_string()), ChildOf(wrapper),
                        InheritedVisibility::VISIBLE,
                        ComputedNode { size: Vec2::new(10.0, 12.0), ..default() })).id());
                }
            }
        }
        TreeFixture { world, root, panel, first_cell: first_cell.unwrap(), icon, text,
            context, model, page }
    }

    fn observe(fixture: &mut TreeFixture) -> BagTreeObservation {
        observe_with_state(fixture, &BagUiState::default())
    }

    fn observe_with_state(fixture: &mut TreeFixture, bag_state: &BagUiState) -> BagTreeObservation {
        let mut state = SystemState::<BagTreeObserver>::new(&mut fixture.world);
        state.get(&fixture.world).expect("tree fixture queries")
            .observe(fixture.root, &fixture.context, &fixture.model, fixture.page,
                bag_state)
    }

    fn source_document() -> CrystalItemTooltipDocument {
        CrystalItemTooltipDocument {
            sections: vec![CrystalItemTooltipSection {
                kind: CrystalItemTooltipSectionKind::Name,
                lines: vec![CrystalItemTooltipLine {
                    text: "WoodenSword".into(), colour: CrystalItemTooltipColour::Yellow,
                }],
            }],
            broken: false, source_complete: true,
        }
    }

    fn painted_in_fixture(fixture: &mut TreeFixture, cell: BagPaintCell) -> Option<BagDetailState> {
        let mut state = SystemState::<(
            Query<Entity, With<BagUiRoot>>,
            Query<(&BagUiTreeBuildStamp, &ChildOf), With<BagUiPanel>>,
            Query<&ChildOf, With<BagPaintGridViewport>>,
            Query<(&BagPaintCell, &CrystalItemHint, &ChildOf)>,
        )>::new(&mut fixture.world);
        let (roots, panels, grids, cells) = state.get(&fixture.world).expect("painted fixture queries");
        painted_detail(&fixture.context, &fixture.model, fixture.page, cell,
            &roots, &panels, &grids, &cells)
    }

    #[test]
    fn detail_pages_preserve_source_lines_colours_and_completeness() {
        let mut document = source_document();
        document.sections.push(CrystalItemTooltipSection {
            kind: CrystalItemTooltipSectionKind::Story,
            lines: (0..12).map(|index| CrystalItemTooltipLine {
                text: format!("{index} 长描述内容重复用于分页和完整性验证 长描述内容重复用于分页和完整性验证"),
                colour: CrystalItemTooltipColour::Plum,
            }).collect(),
        });
        let pages = detail_pages(&document);
        assert!(pages.len() > 2);
        assert!(pages.iter().all(|page| page.source_complete &&
            page.sections.iter().map(|section| section.lines.len()).sum::<usize>() <= 5));
        let source: String = document.sections.iter().flat_map(|section| &section.lines)
            .map(|line| line.text.as_str()).collect();
        let displayed: String = pages.iter().flat_map(|page| &page.sections)
            .flat_map(|section| &section.lines).map(|line| line.text.as_str()).collect();
        assert_eq!(displayed, source);
        assert_eq!(pages[0].sections[0].lines[0].colour, CrystalItemTooltipColour::Yellow);
        assert!(pages.iter().skip(1).flat_map(|page| &page.sections)
            .flat_map(|section| &section.lines).all(|line| line.colour == CrystalItemTooltipColour::Plum));
        document.source_complete = false;
        assert!(detail_pages(&document).iter().all(|page| !page.source_complete));
    }

    #[test]
    fn inspect_reads_only_the_current_painted_item_including_uidless_quest() {
        let mut bag = tree_fixture(BagPage::Bag1, Some((Some(7), 1)));
        bag.world.entity_mut(bag.first_cell).insert(CrystalItemHint(source_document()));
        let cell = BagPaintCell { container: 0, slot: 0, unique_id: Some(7) };
        assert!(painted_in_fixture(&mut bag, cell).is_some());
        assert!(painted_in_fixture(&mut bag, BagPaintCell { unique_id: Some(8), ..cell }).is_none());
        bag.context.identity.owner_revision += 1;
        assert!(painted_in_fixture(&mut bag, cell).is_none());
        let mut quest = tree_fixture(BagPage::Quest, Some((None, 1)));
        quest.world.entity_mut(quest.first_cell).insert(CrystalItemHint(source_document()));
        assert!(painted_in_fixture(&mut quest, BagPaintCell {
            container: 3, slot: 0, unique_id: None,
        }).is_some());
    }

    #[test]
    fn detail_hit_testing_excludes_the_covered_bag_and_world() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        world.spawn((BagUiAction::More, ChildOf(root),
            ComputedNode { size: Vec2::splat(100.0), ..default() },
            bevy::ui::UiGlobalTransform::from_xy(100.0, 100.0),
            InheritedVisibility::VISIBLE));
        let detail = world.spawn((BagUiDetail, ChildOf(root))).id();
        world.spawn((BagUiAction::DetailBack, ChildOf(detail),
            ComputedNode { size: Vec2::splat(100.0), ..default() },
            bevy::ui::UiGlobalTransform::from_xy(100.0, 100.0),
            InheritedVisibility::VISIBLE));
        let mut state = SystemState::<(
            Query<(Entity, &BagUiAction, &ComputedNode, &bevy::ui::UiGlobalTransform,
                &InheritedVisibility)>, Query<&ChildOf>,
        )>::new(&mut world);
        let (actions, parents) = state.get(&world).expect("action fixture queries");
        assert_eq!(action_at(Vec2::new(100.0, 100.0), &actions, &parents,
            Some(detail), true), Some(BagUiAction::DetailBack));
        assert_eq!(action_at(Vec2::new(100.0, 100.0), &actions, &parents,
            None, true), None);
        assert_eq!(action_at(Vec2::new(900.0, 100.0), &actions, &parents,
            Some(detail), true), None);
    }

    #[test]
    fn detail_ready_requires_current_document_text_and_measured_navigation() {
        let mut fixture = tree_fixture(BagPage::Bag1, Some((Some(7), 1)));
        fixture.context.presentation = Some(BagUiPresentation {
            logical_width: 1368.0, logical_height: 768.0,
            stage_css_scale: 640.0 / 1368.0, touch: true,
        });
        let document = source_document();
        fixture.world.entity_mut(fixture.first_cell).insert(CrystalItemHint(document.clone()));
        let stamp = *fixture.world.get::<BagUiTreeBuildStamp>(fixture.panel).unwrap();
        let cell = BagPaintCell { container: 0, slot: 0, unique_id: Some(7) };
        let detail_state = BagDetailState { cell, stamp, document, page: 0 };
        let mut bag_state = BagUiState { page: BagPage::Bag1,
            selected: Some(BagSelection { container: 0, slot: 0, unique_id: 7 }),
            detail: Some(detail_state.clone()), ..default() };
        fixture.world.spawn((BagUiInspector, ChildOf(fixture.root)));
        let detail = fixture.world.spawn((BagUiDetail, detail_state.clone(), stamp,
            ChildOf(fixture.root))).id();
        let body = fixture.world.spawn((BagUiDetailBody, ChildOf(detail), Node::default(),
            ComputedNode { size: Vec2::new(800.0, 300.0), ..default() },
            bevy::ui::UiGlobalTransform::from_xy(500.0, 300.0),
            InheritedVisibility::VISIBLE)).id();
        let section = fixture.world.spawn(ChildOf(body)).id();
        let line = fixture.world.spawn((CrystalItemHintOverlayText, ChildOf(section),
            Text::new("WoodenSword"),
            ComputedNode { size: Vec2::new(300.0, 30.0), ..default() },
            bevy::ui::UiGlobalTransform::from_xy(500.0, 300.0),
            InheritedVisibility::VISIBLE)).id();
        let mut buttons = Vec::new();
        for (index, action) in [BagUiAction::DetailBack,
            BagUiAction::DetailPrev, BagUiAction::DetailNext].into_iter().enumerate() {
            buttons.push(fixture.world.spawn((action, ChildOf(detail), Node::default(),
                ComputedNode { size: Vec2::new(220.0, 88.0), ..default() },
                bevy::ui::UiGlobalTransform::from_xy(120.0 + index as f32 * 350.0, 650.0),
                InheritedVisibility::VISIBLE)).id());
        }
        assert!(observe_with_state(&mut fixture, &bag_state).complete);
        fixture.world.entity_mut(line).get_mut::<ComputedNode>().unwrap().size.y = 0.0;
        assert!(!observe_with_state(&mut fixture, &bag_state).complete,
            "the name alone cannot make an unlaid-out detail ready");
        fixture.world.entity_mut(line).get_mut::<ComputedNode>().unwrap().size.y = 30.0;
        fixture.world.entity_mut(buttons[0]).get_mut::<ComputedNode>().unwrap().size.y = 85.0;
        assert!(!observe_with_state(&mut fixture, &bag_state).complete,
            "85 logical pixels are below 40 CSS at 640/1368");
        fixture.world.entity_mut(buttons[0]).get_mut::<ComputedNode>().unwrap().size.y = 88.0;
        bag_state.detail.as_mut().unwrap().page = 1;
        assert!(!observe_with_state(&mut fixture, &bag_state).complete,
            "the old page tree must not satisfy new detail state");
    }

    #[test]
    fn detail_invalidates_on_owner_model_page_presentation_and_close() {
        let mut context = BagUiHostContext {
            identity: BagIdentity { owner_revision: 6, ..default() },
            model_revision: 4, presentation_revision: 9,
            bag_open: true, page: BagPage::Bag1,
            input_enabled: true, presentation_ready: true,
            presentation: Some(BagUiPresentation {
                logical_width: 1368.0, logical_height: 768.0,
                stage_css_scale: 640.0 / 1368.0, touch: true,
            }),
        };
        let model = BagUiReadModel::default();
        let stamp = BagUiTreeBuildStamp { identity: context.identity,
            model_revision: context.model_revision,
            presentation_revision: context.presentation_revision,
            page: BagPage::Bag1, visible_items: 0 };
        let detail = BagDetailState {
            cell: BagPaintCell { container: 0, slot: 0, unique_id: Some(7) },
            stamp, document: source_document(), page: 0,
        };
        for change in 0..5 {
            let mut state = BagUiState::default();
            state.apply_context(&context, &model);
            state.detail = Some(detail.clone());
            let mut next = context.clone();
            match change {
                0 => next.identity.owner_revision += 1,
                1 => next.model_revision += 1,
                2 => next.page = BagPage::Quest,
                3 => next.presentation_revision += 1,
                _ => next.bag_open = false,
            }
            state.apply_context(&next, &model);
            assert!(state.detail.is_none(), "change {change} must invalidate detail");
        }
        context.input_enabled = false;
        let mut state = BagUiState::default();
        state.detail = Some(detail);
        state.apply_context(&context, &model);
        assert!(state.detail.is_none());
    }

    #[test]
    fn blur_clears_detail_and_held_pointer_without_creating_an_intent() {
        let mut app = fixture();
        app.world_mut().resource_mut::<BagUiHostContext>().presentation = Some(BagUiPresentation {
            logical_width: 1368.0, logical_height: 768.0,
            stage_css_scale: 640.0 / 1368.0, touch: true,
        });
        app.update();
        let context = app.world().resource::<BagUiHostContext>().clone();
        let stamp = BagUiTreeBuildStamp { identity: context.identity,
            model_revision: context.model_revision,
            presentation_revision: context.presentation_revision,
            page: context.page, visible_items: 1 };
        app.world_mut().resource_mut::<BagUiState>().detail = Some(BagDetailState {
            cell: BagPaintCell { container: 0, slot: 0, unique_id: Some(0) },
            stamp, document: source_document(), page: 0,
        });
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&context, 1, BagPointerPhase::Down));
        app.update();
        let mut blur = edge(&context, 2, BagPointerPhase::Blur);
        blur.x = 0.0;
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(blur);
        app.update();
        assert!(app.world().resource::<BagUiState>().detail.is_none());
        assert!(app.world().resource::<PointerState>().lease.is_none());
        assert!(app.world_mut().resource_mut::<BagUiIntentQueue>().drain().is_empty());
    }

    fn touch_tree_fixture(logical_width: f32, css_scale: f32) -> (TreeFixture, Entity, Entity) {
        let mut fixture = tree_fixture(BagPage::Bag1, None);
        let presentation = BagUiPresentation {
            logical_width, logical_height: 768.0, stage_css_scale: css_scale, touch: true,
        };
        fixture.context.presentation = Some(presentation);
        let scale = presentation.paint_layout().scale;
        let panel_left = (logical_width - 448.0 * scale) * 0.5;
        let panel_top = (768.0 - 236.0 * scale) * 0.5;
        let mut first = None;
        let mut second = None;
        let mut cells = fixture.world.query::<(Entity, &BagPaintCell)>();
        let pairs = cells.iter(&fixture.world)
            .map(|(entity, cell)| (entity, cell.slot)).collect::<Vec<_>>();
        for entity in pairs {
            let (entity, slot) = entity;
            let col = slot % 8;
            let row = slot / 8;
            let x = panel_left + (9.0 + col as f32 * 37.0 + 18.0) * scale;
            let y = panel_top + (37.0 + row as f32 * 33.0 + 16.0) * scale;
            fixture.world.entity_mut(entity).insert((
                BagPaintAction::InspectCell { container: 0, slot, unique_id: None },
                Node { width: Val::Px(36.0 * scale), height: Val::Px(32.0 * scale), ..default() },
                ComputedNode { size: Vec2::new(36.0 * scale, 32.0 * scale), ..default() },
                bevy::ui::UiGlobalTransform::from_xy(x, y),
            ));
            if slot == 0 { first = Some(entity); }
            if slot == 1 { second = Some(entity); }
        }
        for page in 0..3u8 {
            let left = panel_left + (6.0 + page as f32 * 73.0) * scale;
            let top = panel_top + 2.0 * scale;
            fixture.world.spawn((BagPaintAction::SelectPage(page), ChildOf(fixture.panel),
                Node { width: Val::Px(72.0 * scale), height: Val::Px(32.0 * scale), ..default() },
                ComputedNode { size: Vec2::new(72.0 * scale, 32.0 * scale), ..default() },
                bevy::ui::UiGlobalTransform::from_xy(left + 36.0 * scale, top + 16.0 * scale),
                InheritedVisibility::VISIBLE));
        }
        fixture.world.spawn((BagPaintAction::Close, ChildOf(fixture.panel),
            Node { width: Val::Px(32.0 * scale), height: Val::Px(32.0 * scale), ..default() },
            ComputedNode { size: Vec2::splat(32.0 * scale), ..default() },
            bevy::ui::UiGlobalTransform::from_xy(panel_left + 294.0 * scale,
                panel_top + 18.0 * scale), InheritedVisibility::VISIBLE));
        let inspector = fixture.world.spawn((BagUiInspector, ChildOf(fixture.root))).id();
        fixture.world.spawn((BagUiAction::More, ChildOf(inspector),
            Node { width: Val::Px(106.0 * scale), height: Val::Px(32.0 * scale), ..default() },
            ComputedNode { size: Vec2::new(106.0 * scale, 32.0 * scale), ..default() },
            bevy::ui::UiGlobalTransform::from_xy(panel_left + (326.0 + 6.0 + 53.0) * scale,
                panel_top + 60.0 * scale), InheritedVisibility::VISIBLE));
        (fixture, first.unwrap(), second.unwrap())
    }

    #[test]
    fn touch_content_ready_checks_computed_hit_bounds_overlap_and_viewport() {
        for (logical_width, css_scale) in [(1368.0, 640.0 / 1368.0),
            (1664.0, 844.0 / 1664.0), (1440.0, 320.0 / 768.0)] {
            let (mut fixture, first, second) = touch_tree_fixture(logical_width, css_scale);
            assert!(observe(&mut fixture).complete);
            fixture.world.entity_mut(first).get_mut::<ComputedNode>().unwrap().size.y *= 0.8;
            assert!(!observe(&mut fixture).complete, "small CSS hit target must fail");
            fixture.world.entity_mut(first).get_mut::<ComputedNode>().unwrap().size.y /= 0.8;
            let first_transform = *fixture.world.get::<bevy::ui::UiGlobalTransform>(first).unwrap();
            fixture.world.entity_mut(second).insert(first_transform);
            assert!(!observe(&mut fixture).complete, "overlapping hit targets must fail");
            fixture.world.entity_mut(second).insert(bevy::ui::UiGlobalTransform::from_xy(
                logical_width + 100.0, 100.0));
            assert!(!observe(&mut fixture).complete, "off-canvas hit target must fail");
            fixture.world.entity_mut(first).remove::<ComputedNode>();
            assert!(!observe(&mut fixture).complete, "unlaid-out action must fail");
        }
    }

    #[test]
    fn touch_renderer_places_panel_inspector_and_more_inside_both_landscapes() {
        for (logical_width, css_scale) in [(1368.0, 640.0 / 1368.0),
            (1664.0, 844.0 / 1664.0), (1440.0, 320.0 / 768.0)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
                .init_asset::<Image>()
                .insert_resource(BagUiHostContext {
                    bag_open: true, input_enabled: true, presentation_ready: true,
                    presentation: Some(BagUiPresentation { logical_width,
                        logical_height: 768.0, stage_css_scale: css_scale, touch: true }),
                    ..default()
                })
                .insert_resource(BagUiReadModel::default())
                .insert_resource(BagUiState::default())
                .insert_resource(QuestUiFont(Handle::default()))
                .add_systems(Update, render_bag_ui);
            app.world_mut().spawn((BagUiRoot, Node::default()));
            app.update();
            let world = app.world_mut();
            let mut panels = world.query_filtered::<&Node, With<BagUiPanel>>();
            let panel = panels.single(world).unwrap();
            let (panel_left, panel_top, panel_width, panel_height) =
                (panel.left, panel.top, panel.width, panel.height);
            let mut inspectors = world.query_filtered::<&Node, With<BagUiInspector>>();
            let inspector = inspectors.single(world).unwrap();
            let (inspector_left_val, inspector_width_val) = (inspector.left, inspector.width);
            let mut actions = world.query::<(&BagUiAction, &Node)>();
            let more = actions.iter(world).find(|(action, _)| **action == BagUiAction::More)
                .expect("renderer must retain More handoff").1;
            let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
                (panel_left, panel_top, panel_width, panel_height) else { panic!("panel px layout"); };
            let (Val::Px(inspector_left), Val::Px(inspector_width)) =
                (inspector_left_val, inspector_width_val) else { panic!("inspector px layout"); };
            assert!(left >= 0.0 && top >= 0.0 && inspector_left + inspector_width <= logical_width);
            assert!(inspector_left >= left + width && top + height <= 768.0);
            assert!(((inspector_left + inspector_width - left) * css_scale - 573.44).abs() < 0.1);
            assert!((height * css_scale - 302.08).abs() < 0.1);
            let (Val::Px(more_width), Val::Px(more_height)) = (more.width, more.height) else { panic!("More px layout"); };
            assert!(more_width * css_scale >= 40.0 && more_height * css_scale >= 39.99);
        }
    }

    #[test]
    fn touch_detail_renderer_keeps_text_and_navigation_inside_three_landscapes() {
        for (logical_width, css_width, css_height) in [
            (1368.0, 640.0, 360.0), (1664.0, 844.0, 390.0),
            (1440.0, 600.0, 320.0),
        ] {
            let css_scale = css_height / 768.0;
            let mut app = App::new();
            let context = BagUiHostContext {
                bag_open: true, input_enabled: true, presentation_ready: true,
                presentation: Some(BagUiPresentation { logical_width,
                    logical_height: 768.0, stage_css_scale: css_scale, touch: true }),
                ..default()
            };
            let detail = BagDetailState {
                cell: BagPaintCell { container: 0, slot: 0, unique_id: Some(7) },
                stamp: BagUiTreeBuildStamp { identity: context.identity,
                    model_revision: context.model_revision,
                    presentation_revision: context.presentation_revision,
                    page: BagPage::Bag1, visible_items: 0 },
                document: source_document(), page: 0,
            };
            app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
                .init_asset::<Image>()
                .insert_resource(context)
                .insert_resource(BagUiReadModel::default())
                .insert_resource(BagUiState { detail: Some(detail), ..default() })
                .insert_resource(QuestUiFont(Handle::default()))
                .add_systems(Update, render_bag_ui);
            app.world_mut().spawn((BagUiRoot, Node::default()));
            app.update();
            let world = app.world_mut();
            let mut details = world.query_filtered::<&Node, With<BagUiDetail>>();
            let panel = details.single(world).unwrap();
            let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
                (panel.left, panel.top, panel.width, panel.height) else { panic!("detail px layout"); };
            assert!(left >= 0.0 && top >= 0.0);
            assert!((left + width) * css_scale <= css_width + 0.2);
            assert!((top + height) * css_scale <= css_height + 0.2);
            let mut buttons = world.query::<(&BagUiAction, &Node)>();
            for action in [BagUiAction::DetailBack, BagUiAction::DetailPrev, BagUiAction::DetailNext] {
                let button = buttons.iter(world).find(|(candidate, _)| **candidate == action)
                    .expect("detail navigation is painted").1;
                let (Val::Px(x), Val::Px(y), Val::Px(w), Val::Px(h)) =
                    (button.left, button.top, button.width, button.height) else { panic!("button px layout"); };
                assert!(w * css_scale >= 40.0 && h * css_scale >= 40.0);
                assert!((left + x + w) * css_scale <= css_width + 0.2);
                assert!((top + y + h) * css_scale <= css_height + 0.2);
            }
            let mut headers = world.query::<(&Text, &TextFont)>();
            let font = headers.iter(world).find(|(text, _)| text.0.starts_with("Item details"))
                .expect("detail page header has font").1;
            let FontSize::Px(font_size) = font.font_size else { panic!("detail font px"); };
            assert!((font_size * css_scale - 14.0).abs() < 0.01);
        }
    }

    #[test]
    fn rounded_eighty_five_and_seventy_eight_pixel_actions_fail_but_new_floor_passes() {
        for (logical_width, css_scale, old_rounded) in [
            (1368.0, 640.0 / 1368.0, 85.0),
            (1664.0, 844.0 / 1664.0, 78.0),
        ] {
            let (mut fixture, first, _) = touch_tree_fixture(logical_width, css_scale);
            fixture.world.entity_mut(first).get_mut::<ComputedNode>().unwrap().size.y = old_rounded;
            let old = observe(&mut fixture);
            assert!(old.content_complete);
            let actions = old.touch_actions.unwrap();
            assert!(!actions.complete);
            let failure = actions.first_failure.unwrap();
            assert_eq!((failure.action_type, failure.slot, failure.reason),
                ("cell", Some(0), "hit_below_40_css"));
            assert!(failure.css_rect.unwrap()[3] < 40.0);
            let rounded_floor = (32.0 * 1.28 / css_scale).floor();
            fixture.world.entity_mut(first).get_mut::<ComputedNode>().unwrap().size.y = rounded_floor;
            let current = observe(&mut fixture);
            assert!(current.content_complete && current.complete);
            let actions = current.touch_actions.unwrap();
            assert!(actions.complete && actions.min_css_hit_height.unwrap() >= 40.0);
            assert_eq!(actions.scoped_action_count, actions.expected_action_count);
        }
    }

    #[test]
    fn observed_tree_requires_current_owner_model_and_presentation() {
        let mut fixture = tree_fixture(BagPage::Bag1, Some((Some(41), 7)));
        assert!(observe(&mut fixture).complete);
        fixture.context.identity.owner_revision += 1;
        assert!(!observe(&mut fixture).complete);
        fixture.context.identity.owner_revision -= 1;
        fixture.context.model_revision += 1;
        assert!(!observe(&mut fixture).complete);
        fixture.context.model_revision -= 1;
        fixture.context.presentation_revision += 1;
        assert!(!observe(&mut fixture).complete);
        fixture.context.presentation_revision -= 1;
        fixture.page = BagPage::Quest;
        assert!(!observe(&mut fixture).complete);
    }

    #[test]
    fn observed_tree_rejects_wrong_uid_slot_and_missing_occupied_marker() {
        let mut fixture = tree_fixture(BagPage::Bag1, Some((Some(41), 7)));
        fixture.world.entity_mut(fixture.first_cell).get_mut::<BagPaintCell>().unwrap().unique_id = Some(42);
        assert!(!observe(&mut fixture).complete);
        fixture.world.entity_mut(fixture.first_cell).get_mut::<BagPaintCell>().unwrap().unique_id = Some(41);
        fixture.world.entity_mut(fixture.first_cell).get_mut::<BagPaintCell>().unwrap().slot = 1;
        assert!(!observe(&mut fixture).complete);
        fixture.world.entity_mut(fixture.first_cell).get_mut::<BagPaintCell>().unwrap().slot = 0;
        fixture.world.entity_mut(fixture.first_cell).remove::<CrystalItemHint>();
        assert!(!observe(&mut fixture).complete);
    }

    #[test]
    fn observed_tree_requires_icon_layout_and_exact_stack_text() {
        let mut fixture = tree_fixture(BagPage::Bag1, Some((Some(41), 7)));
        let icon = fixture.icon.unwrap();
        let text = fixture.text.unwrap();
        fixture.world.entity_mut(icon).insert(ComputedNode::default());
        assert!(!observe(&mut fixture).complete);
        fixture.world.entity_mut(icon).insert(ComputedNode { size: Vec2::new(36.0, 32.0), ..default() });
        let wrong_icon = fixture.world.resource::<AssetServer>().load::<Image>("original-ui/Items/2.png");
        fixture.world.entity_mut(icon).get_mut::<ImageNode>().unwrap().image = wrong_icon;
        assert!(!observe(&mut fixture).complete);
        let right_icon = fixture.world.resource::<AssetServer>().load::<Image>("original-ui/Items/1.png");
        fixture.world.entity_mut(icon).get_mut::<ImageNode>().unwrap().image = right_icon;
        fixture.world.entity_mut(text).get_mut::<Text>().unwrap().0 = "6".into();
        assert!(!observe(&mut fixture).complete);
        fixture.world.entity_mut(text).get_mut::<Text>().unwrap().0 = "7".into();
        fixture.world.despawn(text);
        assert!(!observe(&mut fixture).complete);
    }

    #[test]
    fn observed_tree_accepts_real_empty_bag_and_uidless_read_only_quest_item() {
        let mut empty = tree_fixture(BagPage::Bag1, None);
        let observed = observe(&mut empty);
        assert!(observed.complete);
        assert_eq!((observed.model_visible_items, observed.item_cells, observed.grid_cells), (0, 0, 40));
        empty.world.despawn(empty.first_cell);
        assert!(!observe(&mut empty).complete);
        let mut quest = tree_fixture(BagPage::Quest, Some((None, 1)));
        let observed = observe(&mut quest);
        assert!(observed.complete);
        assert_eq!((observed.model_visible_items, observed.occupied_cells, observed.item_cells, observed.laid_out_icons), (1, 1, 0, 1));
    }

    fn fixture() -> App {
        let mut app = App::new();
        app.insert_resource(BagUiHostContext {
            identity: BagIdentity { run_generation: 1, connection_generation: 1, session_generation: 1, owner_revision: 1 },
            model_revision: 1, presentation_revision: 1, bag_open: true, page: BagPage::Bag1,
            input_enabled: true, presentation_ready: true, presentation: None,
        });
        app.insert_resource(BagUiReadModel {
            inventory: InventoryModel {
                items: vec![crate::inventory::ItemModel {
                    unique_id: Some(0), slot: 0, container: 0,
                    name: "Wooden Sword".into(), ..default()
                }],
                ..default()
            },
            ..default()
        });
        app.init_resource::<BagUiState>()
            .init_resource::<BagUiIntentQueue>()
            .init_resource::<BagUiPointerEdges>()
            .init_resource::<PointerState>()
            .add_systems(Update, (apply_bag_context, process_bag_pointer_edges).chain());
        app.world_mut().spawn((
            BagUiAction::Paint(BagPaintAction::InspectCell { container: 0, slot: 0, unique_id: Some(0) }),
            ComputedNode { size: Vec2::new(100.0, 100.0), inverse_scale_factor: 0.5, ..default() },
            bevy::ui::UiGlobalTransform::from_xy(200.0, 200.0),
            InheritedVisibility::VISIBLE,
        ));
        app.update();
        app
    }

    fn edge(context: &BagUiHostContext, sequence: u64, phase: BagPointerPhase) -> BagUiPointerEdge {
        BagUiPointerEdge {
            identity: context.identity, sequence, presentation_revision: context.presentation_revision,
            pointer_id: 1, phase, origin: BagPointerOrigin::Bag, x: 100.0, y: 100.0, button: 0,
        }
    }

    #[test]
    fn original_owner_and_zero_identity_survive_intent_queue() {
        let mut queue = BagUiIntentQueue::default();
        let context = BagUiHostContext {
            identity: BagIdentity { run_generation: 3, connection_generation: 4, session_generation: 5, owner_revision: 6 },
            model_revision: 7, presentation_revision: 8, bag_open: true, page: BagPage::Bag1,
            input_enabled: true, presentation_ready: true, presentation: None,
        };
        assert!(queue.push(&context, BagUiIntentKind::UseItem {
            source: BagSelection { container: 0, slot: 0, unique_id: 0 },
        }));
        let intent = queue.drain().pop().unwrap();
        assert_eq!(intent.identity.owner_revision, 6);
        assert_eq!(intent.model_revision, 7);
        assert_eq!(intent.presentation_revision, 8);
        assert_eq!(intent.intent_sequence, 1);
        assert!(matches!(intent.kind, BagUiIntentKind::UseItem { source } if source.unique_id == 0));
    }

    #[test]
    fn tree_stamp_distinguishes_old_owner_or_model_from_current_empty_bag() {
        let mut context = BagUiHostContext::default();
        context.identity.owner_revision = 6;
        context.model_revision = 4;
        let mut model = BagUiReadModel::default();
        let old = BagUiTreeBuildStamp {
            identity: context.identity, model_revision: 3, presentation_revision: 0,
            page: BagPage::Bag1,
            visible_items: 0,
        };
        assert!(!old.matches(&context, &model, BagPage::Bag1));
        let current = BagUiTreeBuildStamp { model_revision: 4, ..old };
        assert!(current.matches(&context, &model, BagPage::Bag1));
        model.inventory.items.push(crate::inventory::ItemModel {
            unique_id: Some(1), slot: 0, container: 0, ..default()
        });
        assert!(!current.matches(&context, &model, BagPage::Bag1));
        let populated = BagUiTreeBuildStamp { visible_items: 1, ..current };
        assert!(populated.matches(&context, &model, BagPage::Bag1));
        context.identity.owner_revision = 7;
        assert!(!populated.matches(&context, &model, BagPage::Bag1));
    }

    #[test]
    fn logical_hit_testing_scales_center_and_extent_at_dpr_two() {
        let node = ComputedNode { size: Vec2::new(100.0, 100.0), inverse_scale_factor: 0.5, ..default() };
        let transform = bevy::ui::UiGlobalTransform::from_xy(200.0, 200.0);
        let rect = logical_node_rect(&node, &transform);
        assert_eq!(rect.min, Vec2::new(75.0, 75.0));
        assert_eq!(rect.max, Vec2::new(125.0, 125.0));
        assert!(rect.contains(Vec2::new(120.0, 100.0)));
        assert!(!rect.contains(Vec2::new(140.0, 100.0)));
    }

    #[test]
    fn same_layout_model_refresh_keeps_pointer_and_zero_uid_selection() {
        let mut app = fixture();
        let context = app.world().resource::<BagUiHostContext>().clone();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&context, 1, BagPointerPhase::Down));
        app.update();
        assert_eq!(app.world().resource::<PointerState>().lease.unwrap().pointer_id, 1);
        app.world_mut().resource_mut::<BagUiHostContext>().model_revision = 2;
        app.world_mut().resource_mut::<BagUiReadModel>().player.hp = 10;
        app.update();
        assert!(app.world().resource::<PointerState>().lease.is_some());
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&context, 2, BagPointerPhase::Up));
        app.update();
        assert_eq!(app.world().resource::<BagUiState>().selected.unwrap().unique_id, 0);
    }

    #[test]
    fn desktop_selection_survives_unchanged_frames_and_use_equip_move() {
        let mut app = fixture();
        app.world_mut().resource_mut::<BagUiReadModel>().inventory.items[0].equip_slot = Some("Weapon".into());
        for (x, action) in [(250.0, BagUiAction::Use), (350.0, BagUiAction::Equip),
            (450.0, BagUiAction::Move)] {
            app.world_mut().spawn((action,
                ComputedNode { size: Vec2::new(100.0, 100.0), inverse_scale_factor: 0.5, ..default() },
                bevy::ui::UiGlobalTransform::from_xy(x * 2.0, 200.0),
                InheritedVisibility::VISIBLE));
        }
        let context = app.world().resource::<BagUiHostContext>().clone();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.extend([
            edge(&context, 1, BagPointerPhase::Down), edge(&context, 2, BagPointerPhase::Up),
        ]);
        app.update();
        assert_eq!(app.world().resource::<BagUiState>().selected.unwrap().unique_id, 0);
        app.update();
        assert!(app.world().resource::<BagUiState>().selected.is_some());
        for (sequence, x, expected) in [(3, 250.0, "use"), (5, 350.0, "equip")]
        {
            let mut down = edge(&context, sequence, BagPointerPhase::Down);
            down.x = x;
            let mut up = edge(&context, sequence + 1, BagPointerPhase::Up);
            up.x = x;
            app.world_mut().resource_mut::<BagUiPointerEdges>().0.extend([down, up]);
            app.update();
            let intents = app.world_mut().resource_mut::<BagUiIntentQueue>().drain();
            assert_eq!(intents.len(), 1);
            assert!(matches!((expected, intents[0].kind),
                ("use", BagUiIntentKind::UseItem { .. })
                    | ("equip", BagUiIntentKind::EquipItem { .. })));
            assert!(app.world().resource::<BagUiState>().selected.is_some());
        }
        let mut down = edge(&context, 7, BagPointerPhase::Down);
        down.x = 450.0;
        let mut up = edge(&context, 8, BagPointerPhase::Up);
        up.x = 450.0;
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.extend([down, up]);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<BagUiState>().move_source.unwrap().unique_id, 0);
        assert!(app.world_mut().resource_mut::<BagUiIntentQueue>().drain().is_empty());
    }

    #[test]
    fn old_terminal_cannot_cancel_new_owner_pointer_with_reused_id() {
        let mut app = fixture();
        let old = app.world().resource::<BagUiHostContext>().clone();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&old, 1, BagPointerPhase::Down));
        app.update();
        app.world_mut().resource_mut::<BagUiHostContext>().identity.owner_revision = 2;
        app.update();
        let current = app.world().resource::<BagUiHostContext>().clone();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&current, 1, BagPointerPhase::Down));
        app.update();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&old, 99, BagPointerPhase::Cancel));
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&current, 2, BagPointerPhase::Up));
        app.update();
        assert_eq!(app.world().resource::<BagUiState>().selected.unwrap().unique_id, 0);
    }

    #[test]
    fn world_origin_drag_into_bag_never_selects_or_sends() {
        let mut app = fixture();
        let context = app.world().resource::<BagUiHostContext>().clone();
        let mut down = edge(&context, 1, BagPointerPhase::Down);
        down.origin = BagPointerOrigin::World;
        down.x = 900.0;
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.extend([down, edge(&context, 2, BagPointerPhase::Up)]);
        app.update();
        assert!(app.world().resource::<BagUiState>().selected.is_none());
        assert!(app.world_mut().resource_mut::<BagUiIntentQueue>().drain().is_empty());
    }

    #[test]
    fn drag_to_empty_bag_cell_emits_exact_move_without_second_ledger() {
        let mut app = fixture();
        app.world_mut().spawn((
            BagUiAction::Paint(BagPaintAction::InspectCell { container: 0, slot: 1, unique_id: None }),
            ComputedNode { size: Vec2::new(100.0, 100.0), inverse_scale_factor: 0.5, ..default() },
            bevy::ui::UiGlobalTransform::from_xy(320.0, 200.0),
            InheritedVisibility::VISIBLE,
        ));
        let context = app.world().resource::<BagUiHostContext>().clone();
        let mut release = edge(&context, 2, BagPointerPhase::Up);
        release.x = 160.0; // physical center 320 / DPR 2
        app.world_mut().resource_mut::<BagUiPointerEdges>().0
            .extend([edge(&context, 1, BagPointerPhase::Down), release]);
        app.update();
        let intents = app.world_mut().resource_mut::<BagUiIntentQueue>().drain();
        assert_eq!(intents.len(), 1);
        assert!(matches!(intents[0].kind, BagUiIntentKind::MoveItem {
            source: BagSelection { unique_id: 0, slot: 0, .. }, target_slot: 1
        }));
    }

    #[test]
    fn source_replacement_during_gesture_rejects_old_uid() {
        let mut app = fixture();
        let context = app.world().resource::<BagUiHostContext>().clone();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&context, 1, BagPointerPhase::Down));
        app.update();
        app.world_mut().resource_mut::<BagUiHostContext>().model_revision = 2;
        app.world_mut().resource_mut::<BagUiReadModel>().inventory.items[0].unique_id = Some(1);
        app.update();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&context, 2, BagPointerPhase::Up));
        app.update();
        assert!(app.world().resource::<BagUiState>().selected.is_none());
        assert!(app.world_mut().resource_mut::<BagUiIntentQueue>().drain().is_empty());
    }

    #[test]
    fn old_terminal_before_fresh_down_clears_quarantine() {
        let mut app = fixture();
        let old = app.world().resource::<BagUiHostContext>().clone();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.push(edge(&old, 1, BagPointerPhase::Down));
        app.update();
        app.world_mut().resource_mut::<BagUiHostContext>().identity.owner_revision = 2;
        app.update();
        let current = app.world().resource::<BagUiHostContext>().clone();
        app.world_mut().resource_mut::<BagUiPointerEdges>().0.extend([
            edge(&old, 9, BagPointerPhase::Cancel),
            edge(&current, 1, BagPointerPhase::Down),
            edge(&current, 2, BagPointerPhase::Up),
        ]);
        app.update();
        assert_eq!(app.world().resource::<BagUiState>().selected.unwrap().unique_id, 0);
    }
}
