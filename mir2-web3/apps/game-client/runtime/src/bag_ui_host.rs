//! Independent browser ABI for the shared Crystal bag tree.
//! The host only presents validated snapshots and forwards instance-pinned
//! intentions. It never reserves equipment or sends Gateway commands.

use std::collections::HashSet;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
use bevy::asset::LoadState;
#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;
#[cfg(any(target_arch = "wasm32", test))]
use bevy::ui::{ComputedNode, UiGlobalTransform, UiSystems};
#[cfg(target_arch = "wasm32")]
use js_sys::Function;
use mir2_client_bevy::inventory::InventoryModel;
use mir2_client_bevy::portable_bag_ui::{
    BagIdentity, BagPage, BagPointerOrigin, BagPointerPhase, BagUiHostContext,
    BagUiIntent, BagUiIntentKind, BagUiPointerEdge, BagUiReadModel,
    BagBeltDropGeometry, BagUiPresentation,
};
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::portable_bag_ui::{
    BagUiIntentQueue, BagUiPanel, BagUiInspector, BagUiDetail, BagUiPointerEdges, BagUiRoot,
    BagTreeObserver, BagUiState, Mir2PortableBagUiPlugin, PORTABLE_BAG_REQUIRED_SKINS,
};
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::portable_quest_ui::QuestUiFont;
use mir2_client_bevy::read_model::PlayerStats;
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
#[cfg(target_arch = "wasm32")]
const BAG_FONT_ASSET: &str = "original-ui/fonts/NotoSansCJKsc-Regular.otf";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebBagIdentity {
    run_generation: u64,
    connection_generation: u64,
    session_generation: u64,
    owner_revision: u64,
}

impl WebBagIdentity {
    fn validate(self) -> bool {
        [self.run_generation, self.connection_generation, self.session_generation]
            .into_iter().all(|value| (1..=MAX_SAFE_JS_INTEGER).contains(&value))
            && self.owner_revision <= MAX_SAFE_JS_INTEGER
    }
}

impl From<WebBagIdentity> for BagIdentity {
    fn from(value: WebBagIdentity) -> Self {
        Self {
            run_generation: value.run_generation,
            connection_generation: value.connection_generation,
            session_generation: value.session_generation,
            owner_revision: value.owner_revision,
        }
    }
}

impl From<BagIdentity> for WebBagIdentity {
    fn from(value: BagIdentity) -> Self {
        Self {
            run_generation: value.run_generation,
            connection_generation: value.connection_generation,
            session_generation: value.session_generation,
            owner_revision: value.owner_revision,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebBagPresentation {
    logical_width: f32,
    logical_height: f32,
    stage_css_scale: f32,
    touch: bool,
}

impl From<WebBagPresentation> for BagUiPresentation {
    fn from(value: WebBagPresentation) -> Self {
        Self { logical_width: value.logical_width, logical_height: value.logical_height,
            stage_css_scale: value.stage_css_scale, touch: value.touch }
    }
}

impl WebBagPresentation {
    fn matches_window(self, width: f32, height: f32) -> bool {
        [self.logical_width, self.logical_height, width, height]
            .into_iter().all(|v| v.is_finite() && (1.0..=16_384.0).contains(&v))
            && self.stage_css_scale.is_finite()
            && (0.05..=16.0).contains(&self.stage_css_scale)
            && (self.logical_width - width).abs() <= 1.0
            && (self.logical_height - height).abs() <= 1.0
            && (!self.touch || {
                let css_width = self.logical_width * self.stage_css_scale;
                let css_height = self.logical_height * self.stage_css_scale;
                css_width >= 600.0 && css_height >= 320.0
                    && css_width / css_height >= 1.5
            })
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WebBagPage {
    Bag1,
    Bag2,
    Quest,
}

impl From<WebBagPage> for BagPage {
    fn from(value: WebBagPage) -> Self {
        match value {
            WebBagPage::Bag1 => Self::Bag1,
            WebBagPage::Bag2 => Self::Bag2,
            WebBagPage::Quest => Self::Quest,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebBagSnapshot {
    #[serde(flatten)]
    identity: WebBagIdentity,
    revision: u64,
    model_revision: u64,
    presentation_revision: u64,
    bag_open: bool,
    page: WebBagPage,
    input_enabled: bool,
    presentation: Option<WebBagPresentation>,
    model: InventoryModel,
    player: PlayerStats,
    blocked_unique_ids: Vec<u64>,
    #[serde(default)]
    belt_drop_geometry: Option<BagBeltDropGeometry>,
}

impl WebBagSnapshot {
    fn parse(json: &str) -> Result<Self, &'static str> {
        let value: serde_json::Value = serde_json::from_str(json).map_err(|_| "invalid bag JSON")?;
        // InventoryModel/ItemModel intentionally have legacy serde defaults.
        // The B2 host must not mistake a missing field for authoritative empty.
        let root = value.as_object().ok_or("invalid bag snapshot")?;
        let model = root.get("model").and_then(serde_json::Value::as_object).ok_or("missing bag model")?;
        for key in ["capacity", "gold", "items"] {
            if !model.contains_key(key) { return Err("incomplete bag model"); }
        }
        let items = model.get("items").and_then(serde_json::Value::as_array).ok_or("invalid bag items")?;
        for item in items {
            let item = item.as_object().ok_or("invalid bag item")?;
            for key in ["uniqueId", "key", "name", "quantity", "slot", "container", "icon", "description"] {
                if !item.contains_key(key) { return Err("incomplete bag item"); }
            }
        }
        if let Some(geometry) = root.get("beltDropGeometry").filter(|value| !value.is_null()) {
            let geometry = geometry.as_object().ok_or("invalid belt geometry")?;
            let targets = geometry.get("targets").and_then(serde_json::Value::as_array)
                .ok_or("missing belt targets")?;
            for target in targets {
                let target = target.as_object().ok_or("invalid belt target")?;
                if !target.contains_key("uniqueId") { return Err("missing belt target identity"); }
            }
        }
        let snapshot: Self = serde_json::from_value(value).map_err(|_| "invalid bag fields")?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn validate(&self) -> Result<(), &'static str> {
        if !self.identity.validate() || [self.revision, self.model_revision, self.presentation_revision]
            .into_iter().any(|v| v > MAX_SAFE_JS_INTEGER)
        { return Err("invalid bag identity or revision"); }
        if self.model.capacity != InventoryModel::canonical_capacity(self.model.capacity) {
            return Err("invalid bag capacity");
        }
        if self.model.items.len() > 160 { return Err("bag model too large"); }
        let capacity = u32::from(self.model.bag_slot_capacity());
        let mut cells = HashSet::new();
        let mut ids = HashSet::new();
        for item in &self.model.items {
            if item.quantity == 0 || item.key.len() > 256 || item.name.len() > 256 {
                return Err("invalid bag item data");
            }
            let valid_slot = match item.container {
                0 => item.slot < capacity,
                1 => item.slot < 6,
                2 => item.slot < 14,
                3 => item.slot < 40,
                _ => false,
            };
            if !valid_slot || !cells.insert((item.container, item.slot)) {
                return Err("invalid or duplicate bag cell");
            }
            if let Some(id) = item.unique_id {
                if id > MAX_SAFE_JS_INTEGER || !ids.insert(id) {
                    return Err("invalid or duplicate bag identity");
                }
            }
        }
        if self.blocked_unique_ids.len() > 128 { return Err("too many blocked items"); }
        let mut blocked = HashSet::new();
        for id in &self.blocked_unique_ids {
            if *id > MAX_SAFE_JS_INTEGER || !blocked.insert(*id) {
                return Err("invalid blocked item identity");
            }
        }
        if let Some(geometry) = &self.belt_drop_geometry {
            if !geometry.matches_inventory_and_stage(&self.model, self.presentation.map(|p| p.into())) {
                return Err("invalid belt geometry or inventory identity");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebBagPointerEdge {
    #[serde(flatten)]
    identity: WebBagIdentity,
    sequence: u64,
    presentation_revision: u64,
    pointer_id: u64,
    phase: WebBagPointerPhase,
    origin: WebBagPointerOrigin,
    x: f32,
    y: f32,
    button: u8,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WebBagPointerPhase { Down, Move, Up, Cancel, Blur }
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WebBagPointerOrigin { Bag, World }

impl WebBagPointerEdge {
    fn validate(self) -> bool {
        self.identity.validate()
            && (1..=MAX_SAFE_JS_INTEGER).contains(&self.sequence)
            && self.presentation_revision <= MAX_SAFE_JS_INTEGER
            && self.pointer_id <= MAX_SAFE_JS_INTEGER
            && [self.x, self.y].into_iter().all(|v| v.is_finite() && v.abs() <= 16_384.0)
            && matches!(self.button, 0 | 2)
    }

    fn into_shared(self) -> BagUiPointerEdge {
        BagUiPointerEdge {
            identity: self.identity.into(),
            sequence: self.sequence,
            presentation_revision: self.presentation_revision,
            pointer_id: self.pointer_id,
            phase: match self.phase {
                WebBagPointerPhase::Down => BagPointerPhase::Down,
                WebBagPointerPhase::Move => BagPointerPhase::Move,
                WebBagPointerPhase::Up => BagPointerPhase::Up,
                WebBagPointerPhase::Cancel => BagPointerPhase::Cancel,
                WebBagPointerPhase::Blur => BagPointerPhase::Blur,
            },
            origin: match self.origin {
                WebBagPointerOrigin::Bag => BagPointerOrigin::Bag,
                WebBagPointerOrigin::World => BagPointerOrigin::World,
            },
            x: self.x, y: self.y, button: self.button,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BagInputRegion { left: f32, top: f32, width: f32, height: f32 }

impl BagInputRegion {
    fn right(self) -> f32 { self.left + self.width }
    fn bottom(self) -> f32 { self.top + self.height }
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && x < self.right() && y >= self.top && y < self.bottom()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebBagStatus {
    #[serde(flatten)]
    identity: WebBagIdentity,
    frame: u64,
    ready: bool,
    input_enabled: bool,
    applied_revision: u64,
    applied_model_revision: u64,
    applied_presentation_revision: u64,
    supports_bag_to_belt: bool,
    applied_belt_geometry_revision: Option<u64>,
    input_regions: Vec<BagInputRegion>,
    diagnostics: BagTreeDiagnostics,
    error: Option<String>,
}

/// Read-only evidence for the applied model and the actual Bag grid subtree.
/// Counts are scoped to this host's current grid; no item identifiers leave WASM.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct BagTreeDiagnostics {
    model_visible_items: usize,
    tree_content_complete: Option<bool>,
    touch_actions_complete: Option<bool>,
    scoped_action_count: Option<usize>,
    expected_action_count: Option<usize>,
    min_css_hit_width: Option<f32>,
    min_css_hit_height: Option<f32>,
    first_action_failure: Option<BagActionFailureStatus>,
    layout_matches_window: bool,
    assets_loading: bool,
    layout_region_count: usize,
    sink_present: bool,
    tree_identity: Option<WebBagIdentity>,
    tree_model_revision: Option<u64>,
    tree_page: Option<&'static str>,
    tree_visible_items: Option<usize>,
    tree_matches_model: bool,
    grid_cells: usize,
    item_cells: usize,
    occupied_cells: usize,
    icon_nodes: usize,
    visible_icons: usize,
    laid_out_icons: usize,
    stack_text_nodes: usize,
    visible_stack_texts: usize,
    laid_out_stack_texts: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BagActionFailureStatus {
    action_type: &'static str,
    slot: Option<u32>,
    reason: &'static str,
    css_rect: Option<BagInputRegion>,
}

impl Default for WebBagStatus {
    fn default() -> Self {
        Self {
            identity: WebBagIdentity::default(), frame: 0, ready: false,
            input_enabled: false, applied_revision: 0, applied_model_revision: 0,
            applied_presentation_revision: 0, supports_bag_to_belt: true,
            applied_belt_geometry_revision: None, input_regions: Vec::new(),
            diagnostics: BagTreeDiagnostics::default(), error: None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
struct AppliedBagSnapshot {
    identity: WebBagIdentity,
    revision: u64,
    model_revision: u64,
    presentation_revision: u64,
    bag_open: bool,
    input_enabled: bool,
    presentation: Option<WebBagPresentation>,
    has_snapshot: bool,
    icon_paths: Vec<String>,
    icon_metadata_error: bool,
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
struct BagHostAssets {
    font: Option<Handle<Font>>,
    skins: Vec<Handle<Image>>,
    icons: Vec<Handle<Image>>,
    requested: bool,
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
struct BagHostWindow(Entity);

#[cfg(target_arch = "wasm32")]
thread_local! {
    static PENDING_SNAPSHOT: RefCell<Option<WebBagSnapshot>> = const { RefCell::new(None) };
    static PENDING_EDGES: RefCell<Vec<BagUiPointerEdge>> = const { RefCell::new(Vec::new()) };
    static INTENT_SINK: RefCell<Option<Function>> = const { RefCell::new(None) };
    static STATUS: RefCell<WebBagStatus> = RefCell::new(WebBagStatus::default());
    static REJECTED_SNAPSHOT: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[cfg(all(target_arch = "wasm32", feature = "webgl2", not(feature = "webgpu")))]
pub(crate) fn install_unsupported(app: &mut App) {
    app.add_systems(Update, || {
        STATUS.with(|status| {
            let mut status = status.borrow_mut();
            status.ready = false;
            status.input_enabled = false;
            status.error = Some("Shared bag is unavailable in this WebGL2 runtime".to_owned());
        });
    });
}

#[cfg(all(target_arch = "wasm32", not(all(feature = "webgl2",
    not(feature = "webgpu"), not(feature = "webgl2-shared-ui")))))]
pub(crate) fn install(app: &mut App, window: Entity, _camera: Entity) {
    use mir2_client_bevy::pending_operations::PendingLifecycleSet;
    app.insert_resource(BagHostWindow(window))
        .init_resource::<AppliedBagSnapshot>()
        .init_resource::<BagHostAssets>()
        .add_plugins(Mir2PortableBagUiPlugin)
        .add_systems(
            Update,
            (ingest_snapshot, sync_presentation, ingest_pointer_edges)
                .chain().in_set(PendingLifecycleSet::Ingest),
        )
        .add_systems(PostUpdate, publish_status.after(UiSystems::Layout))
        .add_systems(Last, forward_intents);
}

#[cfg(any(target_arch = "wasm32", test))]
fn visible_icons(model: &InventoryModel, page: BagPage) -> Result<Vec<String>, &'static str> {
    let mut paths = Vec::new();
    for item in &model.items {
        let visible = match page {
            BagPage::Bag1 => item.container == 0 && item.slot < 40,
            BagPage::Bag2 => item.container == 0 && (40..80).contains(&item.slot),
            BagPage::Quest => item.container == 3 && item.slot < 40,
        };
        if !visible { continue; }
        let index = item.user_item_image_index().ok_or("bag item icon metadata unavailable")?;
        paths.push(format!("original-ui/Items/{index}.png"));
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

#[cfg(target_arch = "wasm32")]
fn ingest_snapshot(
    mut applied: ResMut<AppliedBagSnapshot>,
    mut context: ResMut<BagUiHostContext>,
    mut read: ResMut<BagUiReadModel>,
    mut assets: ResMut<BagHostAssets>,
    mut font: ResMut<QuestUiFont>,
    server: Res<AssetServer>,
    mut queue: ResMut<BagUiIntentQueue>,
) {
    let Some(snapshot) = PENDING_SNAPSHOT.with(|cell| cell.borrow_mut().take()) else { return; };
    if applied.has_snapshot && (snapshot.identity.run_generation < applied.identity.run_generation
        || snapshot.identity.run_generation == applied.identity.run_generation
            && snapshot.revision <= applied.revision) { return; }
    let identity: BagIdentity = snapshot.identity.into();
    if applied.has_snapshot && snapshot.identity != applied.identity {
        queue.clear();
    }
    if snapshot.bag_open && !assets.requested {
        let handle = server.load::<Font>(BAG_FONT_ASSET);
        font.0 = handle.clone();
        assets.font = Some(handle);
        assets.skins = PORTABLE_BAG_REQUIRED_SKINS.iter()
            .map(|path| server.load::<Image>(*path)).collect();
        assets.requested = true;
    }
    let page: BagPage = snapshot.page.into();
    let paths = visible_icons(&snapshot.model, page);
    match paths {
        Ok(paths) => {
            if paths != applied.icon_paths {
                assets.icons = paths.iter().map(|path| server.load::<Image>(path.clone())).collect();
                applied.icon_paths = paths;
            }
            applied.icon_metadata_error = false;
        }
        Err(_) => {
            assets.icons.clear();
            applied.icon_paths.clear();
            applied.icon_metadata_error = true;
        }
    }
    *read = BagUiReadModel {
        inventory: snapshot.model,
        player: snapshot.player,
        blocked_unique_ids: snapshot.blocked_unique_ids.into_iter().collect(),
        belt_drop_geometry: snapshot.belt_drop_geometry,
    };
    context.identity = identity;
    context.model_revision = snapshot.model_revision;
    context.presentation_revision = snapshot.presentation_revision;
    context.bag_open = snapshot.bag_open;
    context.page = page;
    context.input_enabled = snapshot.input_enabled;
    context.presentation = snapshot.presentation.map(|metrics| BagUiPresentation {
        logical_width: metrics.logical_width,
        logical_height: metrics.logical_height,
        stage_css_scale: metrics.stage_css_scale,
        touch: metrics.touch,
    });
    // The geometry check runs in this same Ingest set before bag input. A
    // routine model/HP/pending update must never create a false layout edge.
    applied.identity = snapshot.identity;
    applied.revision = snapshot.revision;
    applied.model_revision = snapshot.model_revision;
    applied.presentation_revision = snapshot.presentation_revision;
    applied.bag_open = snapshot.bag_open;
    applied.input_enabled = snapshot.input_enabled;
    applied.presentation = snapshot.presentation;
    applied.has_snapshot = true;
}

#[cfg(target_arch = "wasm32")]
fn sync_presentation(
    applied: Res<AppliedBagSnapshot>,
    surface: Res<BagHostWindow>,
    windows: Query<&Window>,
    mut context: ResMut<BagUiHostContext>,
) {
    let valid = applied.presentation.zip(windows.get(surface.0).ok())
        .is_some_and(|(metrics, window)| metrics.matches_window(window.width(), window.height()));
    if context.presentation_ready != valid { context.presentation_ready = valid; }
}

#[cfg(target_arch = "wasm32")]
fn ingest_pointer_edges(mut edges: ResMut<BagUiPointerEdges>) {
    PENDING_EDGES.with(|pending| edges.0.extend(std::mem::take(&mut *pending.borrow_mut())));
}

#[cfg(any(target_arch = "wasm32", test))]
fn node_region(node: &ComputedNode, transform: &UiGlobalTransform) -> Option<BagInputRegion> {
    let scale = node.inverse_scale_factor;
    let size = node.size() * scale;
    let center = transform.affine().translation * scale;
    if !size.is_finite() || !center.is_finite() || size.x <= 0.0 || size.y <= 0.0 { return None; }
    Some(BagInputRegion {
        left: center.x - size.x * 0.5,
        top: center.y - size.y * 0.5,
        width: size.x, height: size.y,
    })
}

/// A visible detail is a third, measured input region bridging the otherwise
/// world-owned gap between the two ordinary Bag surfaces. Missing/stale or
/// unlaid-out detail geometry cannot satisfy the status readiness contract.
#[cfg(any(target_arch = "wasm32", test))]
fn published_input_regions(
    panels: Vec<BagInputRegion>, inspectors: Vec<BagInputRegion>,
    details: Vec<BagInputRegion>, detail_active: bool,
    stage: Option<(f32, f32)>,
) -> Option<Vec<BagInputRegion>> {
    let ([panel], [inspector]) = (panels.as_slice(), inspectors.as_slice()) else { return None; };
    if !detail_active {
        return details.is_empty().then_some(vec![*panel, *inspector]);
    }
    let [detail] = details.as_slice() else { return None; };
    let (stage_width, stage_height) = stage?;
    let gap_left = panel.right();
    let gap_right = inspector.left;
    let gap_top = panel.top.max(inspector.top);
    let gap_bottom = panel.bottom().min(inspector.bottom());
    // The detail paint covers the complete two-surface footprint, including
    // footer buttons below the inspector's content-driven height. One logical
    // pixel allows the independent Taffy rounds of sibling boxes; the two
    // original measured regions remain published for those boundary pixels.
    let round_tolerance = 1.0;
    if gap_right <= gap_left || gap_bottom <= gap_top
        || detail.left > gap_left || detail.right() < gap_right
        || detail.top > gap_top || detail.bottom() < gap_bottom
        || detail.left > panel.left.min(inspector.left) + round_tolerance
        || detail.top > panel.top.min(inspector.top) + round_tolerance
        || detail.right() + round_tolerance < panel.right().max(inspector.right())
        || detail.bottom() + round_tolerance < panel.bottom().max(inspector.bottom())
        || detail.left < -0.1 || detail.top < -0.1
        || detail.right() > stage_width + 0.1
        || detail.bottom() > stage_height + 0.1 {
        return None;
    }
    Some(vec![*panel, *inspector, *detail])
}

#[cfg(target_arch = "wasm32")]
fn publish_status(
    applied: Res<AppliedBagSnapshot>,
    context: Res<BagUiHostContext>,
    model: Res<BagUiReadModel>,
    page: Res<BagUiState>,
    assets: Res<BagHostAssets>,
    server: Res<AssetServer>,
    surface: Res<BagHostWindow>,
    windows: Query<&Window>,
    roots: Query<Entity, With<BagUiRoot>>,
    panel: Query<(&ComputedNode, &UiGlobalTransform), With<BagUiPanel>>,
    inspector: Query<(&ComputedNode, &UiGlobalTransform), With<BagUiInspector>>,
    detail: Query<(&ComputedNode, &UiGlobalTransform), With<BagUiDetail>>,
    tree: BagTreeObserver,
) {
    let layout_ok = applied.presentation.zip(windows.get(surface.0).ok())
        .is_some_and(|(metrics, window)| metrics.matches_window(window.width(), window.height()));
    let mut loading = !assets.requested;
    let mut failed = false;
    for state in assets.font.iter().map(|h| server.get_load_state(h.id()))
        .chain(assets.skins.iter().map(|h| server.get_load_state(h.id())))
        .chain(assets.icons.iter().map(|h| server.get_load_state(h.id())))
    {
        match state {
            Some(LoadState::Loaded) => {}
            Some(LoadState::Failed(_)) => failed = true,
            _ => loading = true,
        }
    }
    let regions = published_input_regions(
        panel.iter().filter_map(|(node, transform)| node_region(node, transform)).collect(),
        inspector.iter().filter_map(|(node, transform)| node_region(node, transform)).collect(),
        detail.iter().filter_map(|(node, transform)| node_region(node, transform)).collect(),
        page.detail_active(),
        applied.presentation.map(|metrics| (metrics.logical_width, metrics.logical_height)),
    );
    let region_ready = regions.is_some();
    let regions = regions.unwrap_or_default();
    let sink_present = INTENT_SINK.with(|sink| sink.borrow().is_some());
    let observation = roots.single().ok().map(|root| tree.observe(root, &context, &model, page.page, &page));
    let mut diagnostics = BagTreeDiagnostics {
        model_visible_items: mir2_client_bevy::portable_bag_ui::visible_bag_item_count(
            &model.inventory, page.page),
        layout_matches_window: layout_ok,
        assets_loading: loading,
        layout_region_count: regions.len(),
        sink_present,
        ..Default::default()
    };
    if let Some(stamp) = observation.as_ref().and_then(|tree| tree.stamp) {
        diagnostics.tree_identity = Some(stamp.identity.into());
        diagnostics.tree_model_revision = Some(stamp.model_revision);
        diagnostics.tree_page = Some(match stamp.page {
            BagPage::Bag1 => "bag1", BagPage::Bag2 => "bag2", BagPage::Quest => "quest",
        });
        diagnostics.tree_visible_items = Some(stamp.visible_items);
        diagnostics.tree_matches_model = observation.as_ref().is_some_and(|tree| tree.stamp_matches_model);
    }
    if let Some(tree) = observation.as_ref() {
        diagnostics.tree_content_complete = Some(tree.content_complete);
        if let Some(actions) = &tree.touch_actions {
            diagnostics.touch_actions_complete = Some(actions.complete);
            diagnostics.scoped_action_count = Some(actions.scoped_action_count);
            diagnostics.expected_action_count = Some(actions.expected_action_count);
            diagnostics.min_css_hit_width = actions.min_css_hit_width;
            diagnostics.min_css_hit_height = actions.min_css_hit_height;
            diagnostics.first_action_failure = actions.first_failure.map(|failure| {
                BagActionFailureStatus {
                    action_type: failure.action_type,
                    slot: failure.slot,
                    reason: failure.reason,
                    css_rect: failure.css_rect.map(|rect| BagInputRegion {
                        left: rect[0], top: rect[1], width: rect[2], height: rect[3],
                    }),
                }
            });
        }
        diagnostics.grid_cells = tree.grid_cells;
        diagnostics.item_cells = tree.item_cells;
        diagnostics.occupied_cells = tree.occupied_cells;
        diagnostics.icon_nodes = tree.icon_nodes;
        diagnostics.visible_icons = tree.visible_icons;
        diagnostics.laid_out_icons = tree.laid_out_icons;
        diagnostics.stack_text_nodes = tree.stack_text_nodes;
        diagnostics.visible_stack_texts = tree.visible_stack_texts;
        diagnostics.laid_out_stack_texts = tree.laid_out_stack_texts;
    }
    let tree_complete = observation.is_some_and(|tree| tree.complete);
    STATUS.with(|cell| {
        let mut status = cell.borrow_mut();
        status.frame = status.frame.saturating_add(1).min(MAX_SAFE_JS_INTEGER);
        status.identity = applied.identity;
        status.applied_revision = applied.revision;
        status.applied_model_revision = applied.model_revision;
        status.applied_presentation_revision = applied.presentation_revision;
        status.applied_belt_geometry_revision = model.belt_drop_geometry.as_ref().map(|g| g.revision);
        status.diagnostics = diagnostics;
        status.error = REJECTED_SNAPSHOT.with(|error| error.borrow().clone())
            .or_else(|| applied.icon_metadata_error.then(|| "bag item icon metadata unavailable".to_owned()))
            .or_else(|| failed.then(|| "bag font, skin or visible item icon failed to load".to_owned()))
            .or_else(|| (applied.has_snapshot && applied.bag_open && !layout_ok)
                .then(|| "bag presentation is unavailable".to_owned()));
        status.ready = applied.has_snapshot && applied.bag_open && layout_ok && !loading
            && !failed && roots.single().is_ok() && region_ready && tree_complete
            && sink_present
            && status.error.is_none();
        status.input_enabled = status.ready && applied.input_enabled && context.input_enabled;
        status.input_regions = if status.ready { regions } else { Vec::new() };
    });
}

#[cfg(any(target_arch = "wasm32", test))]
fn encode_intent(intent: BagUiIntent) -> String {
    use serde_json::json;
    let identity: WebBagIdentity = intent.identity.into();
    let mut value = json!({
        "runGeneration": identity.run_generation,
        "connectionGeneration": identity.connection_generation,
        "sessionGeneration": identity.session_generation,
        "ownerRevision": identity.owner_revision,
        "intentSequence": intent.intent_sequence,
        "modelRevision": intent.model_revision,
        "presentationRevision": intent.presentation_revision,
    });
    let object = value.as_object_mut().expect("object literal");
    match intent.kind {
        BagUiIntentKind::UseItem { source } => {
            object.insert("type".to_owned(), json!("useItem"));
            object.insert("source".to_owned(), json!({"container": 0, "slot": source.slot, "uniqueId": source.unique_id}));
        }
        BagUiIntentKind::EquipItem { source } => {
            object.insert("type".to_owned(), json!("equipItem"));
            object.insert("source".to_owned(), json!({"container": 0, "slot": source.slot, "uniqueId": source.unique_id}));
        }
        BagUiIntentKind::MoveItem { source, target_slot } => {
            object.insert("type".to_owned(), json!("moveItem"));
            object.insert("source".to_owned(), json!({"container": 0, "slot": source.slot, "uniqueId": source.unique_id}));
            object.insert("target".to_owned(), json!({"container": 0, "slot": target_slot}));
        }
        BagUiIntentKind::MoveToBelt { source, target_slot, target_unique_id, belt_geometry_revision } => {
            object.insert("type".to_owned(), json!("moveToBelt"));
            object.insert("source".to_owned(), json!({"container": 0, "slot": source.slot, "uniqueId": source.unique_id}));
            object.insert("target".to_owned(), json!({"container": 1, "slot": target_slot, "uniqueId": target_unique_id}));
            object.insert("beltGeometryRevision".to_owned(), json!(belt_geometry_revision));
        }
        BagUiIntentKind::Close => { object.insert("type".to_owned(), json!("close")); }
        BagUiIntentKind::SelectPage(page) => {
            object.insert("type".to_owned(), json!("selectPage"));
            object.insert("page".to_owned(), json!(match page {
                BagPage::Bag1 => "bag1", BagPage::Bag2 => "bag2", BagPage::Quest => "quest",
            }));
        }
        BagUiIntentKind::HandoffFullInventory | BagUiIntentKind::HandoffDelete => {
            object.insert("type".to_owned(), json!("handoff"));
            object.insert("mode".to_owned(), json!(if matches!(intent.kind, BagUiIntentKind::HandoffDelete) {
                "delete"
            } else { "fullInventory" }));
        }
    }
    value.to_string()
}

#[cfg(target_arch = "wasm32")]
fn forward_intents(mut queue: ResMut<BagUiIntentQueue>) {
    let intents = queue.drain();
    if intents.is_empty() { return; }
    let sink = INTENT_SINK.with(|sink| sink.borrow().clone());
    let active = STATUS.with(|cell| cell.borrow().clone());
    for intent in intents {
        if !active.input_enabled || WebBagIdentity::from(intent.identity) != active.identity
            || intent.model_revision != active.applied_model_revision
            || intent.presentation_revision != active.applied_presentation_revision
        { continue; }
        if let BagUiIntentKind::MoveToBelt { belt_geometry_revision, .. } = intent.kind {
            if active.applied_belt_geometry_revision != Some(belt_geometry_revision) { continue; }
        }
        if let Some(sink) = sink.as_ref() {
            let _ = sink.call1(&JsValue::NULL, &JsValue::from_str(&encode_intent(intent)));
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = getMir2BagUiAbiVersion)]
pub fn get_mir2_bag_ui_abi_version() -> u32 { 1 }

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = setMir2BagUiSnapshot)]
pub fn set_mir2_bag_ui_snapshot(json: String) -> bool {
    let snapshot = match WebBagSnapshot::parse(&json) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            REJECTED_SNAPSHOT.with(|rejected| *rejected.borrow_mut() = Some(error.to_owned()));
            STATUS.with(|status| {
                let mut status = status.borrow_mut();
                status.ready = false;
                status.input_enabled = false;
                status.error = Some(error.to_owned());
                status.input_regions.clear();
            });
            return false;
        }
    };
    let stale = STATUS.with(|status| {
        let status = status.borrow();
        snapshot.identity.run_generation < status.identity.run_generation
            || snapshot.identity.run_generation == status.identity.run_generation
                && snapshot.revision <= status.applied_revision
    }) || PENDING_SNAPSHOT.with(|pending| {
        pending.borrow().as_ref().is_some_and(|old| {
            snapshot.identity.run_generation < old.identity.run_generation
                || snapshot.identity.run_generation == old.identity.run_generation
                    && snapshot.revision <= old.revision
        })
    });
    if stale { return false; }
    REJECTED_SNAPSHOT.with(|rejected| *rejected.borrow_mut() = None);
    PENDING_SNAPSHOT.with(|pending| *pending.borrow_mut() = Some(snapshot));
    true
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = getMir2BagUiStatus)]
pub fn get_mir2_bag_ui_status() -> String {
    STATUS.with(|status| serde_json::to_string(&*status.borrow()).unwrap_or_default())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = setMir2BagUiIntentSink)]
pub fn set_mir2_bag_ui_intent_sink(sink: Function) {
    INTENT_SINK.with(|cell| *cell.borrow_mut() = Some(sink));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = clearMir2BagUiIntentSink)]
pub fn clear_mir2_bag_ui_intent_sink() {
    INTENT_SINK.with(|cell| *cell.borrow_mut() = None);
    STATUS.with(|cell| {
        let mut status = cell.borrow_mut();
        status.ready = false;
        status.input_enabled = false;
        status.input_regions.clear();
    });
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = setMir2BagUiPointerEdge)]
pub fn set_mir2_bag_ui_pointer_edge(json: String) -> bool {
    let Ok(edge) = serde_json::from_str::<WebBagPointerEdge>(&json) else { return false; };
    if !edge.validate() { return false; }
    let cleanup = matches!(edge.phase, WebBagPointerPhase::Up | WebBagPointerPhase::Cancel | WebBagPointerPhase::Blur);
    let current = STATUS.with(|cell| {
        let status = cell.borrow();
        status.input_enabled && status.identity == edge.identity
            && status.applied_presentation_revision == edge.presentation_revision
    }) && PENDING_SNAPSHOT.with(|pending| {
        pending.borrow().as_ref().is_none_or(|newer| newer.identity == edge.identity
            && newer.presentation_revision == edge.presentation_revision)
    });
    if !current && !cleanup { return false; }
    PENDING_EDGES.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.len() >= 256 { return false; }
        pending.push(edge.into_shared());
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_failure_diagnostics_serialize_geometry_without_item_identity() {
        let diagnostics = BagTreeDiagnostics {
            tree_content_complete: Some(true),
            touch_actions_complete: Some(false),
            scoped_action_count: Some(45),
            expected_action_count: Some(45),
            min_css_hit_width: Some(40.0),
            min_css_hit_height: Some(39.766),
            first_action_failure: Some(BagActionFailureStatus {
                action_type: "cell", slot: Some(0), reason: "hit_below_40_css",
                css_rect: Some(BagInputRegion {
                    left: 46.0, top: 79.0, width: 45.0, height: 39.766,
                }),
            }),
            layout_matches_window: true, assets_loading: false,
            layout_region_count: 2, sink_present: true,
            ..Default::default()
        };
        let value = serde_json::to_value(&diagnostics).unwrap();
        assert_eq!(value["treeContentComplete"], true);
        assert_eq!(value["touchActionsComplete"], false);
        assert_eq!(value["firstActionFailure"]["reason"], "hit_below_40_css");
        assert_eq!(value["firstActionFailure"]["slot"], 0);
        assert!(value.get("uniqueId").is_none());
        assert!(value["firstActionFailure"].get("uniqueId").is_none());
    }
    use serde_json::json;

    fn valid() -> serde_json::Value {
        json!({
            "runGeneration":1,"connectionGeneration":2,"sessionGeneration":3,"ownerRevision":4,
            "revision":5,"modelRevision":6,"presentationRevision":7,"bagOpen":true,
            "page":"bag1","inputEnabled":false,
            "presentation":{"logicalWidth":1024.0,"logicalHeight":768.0,"stageCssScale":1.0,"touch":false},
            "model":{"capacity":46,"gold":0,"items":[{
                "uniqueId":0,"key":"0","name":"Wooden Sword","quantity":1,"slot":0,"container":0,
                "icon":1,"description":""
            }]},
            "player":{"level":1},"blockedUniqueIds":[0]
        })
    }

    #[test]
    fn strict_complete_model_accepts_zero_and_rejects_missing_fields() {
        assert!(WebBagSnapshot::parse(&valid().to_string()).is_ok());
        let mut missing = valid();
        missing["model"].as_object_mut().unwrap().remove("capacity");
        assert!(WebBagSnapshot::parse(&missing.to_string()).is_err());
        let mut missing_id = valid();
        missing_id["model"]["items"][0].as_object_mut().unwrap().remove("uniqueId");
        assert!(WebBagSnapshot::parse(&missing_id.to_string()).is_err());
    }

    #[test]
    fn duplicate_cells_ids_and_capacity_fail_closed() {
        let mut duplicate_cell = valid();
        duplicate_cell["model"]["items"].as_array_mut().unwrap().push(json!({
            "uniqueId":1,"key":"1","name":"Other","quantity":1,"slot":0,"container":0,
            "icon":2,"description":""
        }));
        assert!(WebBagSnapshot::parse(&duplicate_cell.to_string()).is_err());
        let mut duplicate_id = duplicate_cell.clone();
        duplicate_id["model"]["items"][1]["slot"] = json!(1);
        duplicate_id["model"]["items"][1]["uniqueId"] = json!(0);
        assert!(WebBagSnapshot::parse(&duplicate_id.to_string()).is_err());
        let mut capacity = valid();
        capacity["model"]["capacity"] = json!(47);
        assert!(WebBagSnapshot::parse(&capacity.to_string()).is_err());
    }

    #[test]
    fn published_input_region_uses_logical_dpr_two_coordinates() {
        let node = ComputedNode { size: bevy::math::Vec2::new(632.0, 472.0),
            inverse_scale_factor: 0.5, ..Default::default() };
        let transform = UiGlobalTransform::from_xy(1136.0, 408.0);
        let region = node_region(&node, &transform).unwrap();
        assert_eq!((region.left, region.top, region.width, region.height), (410.0, 86.0, 316.0, 236.0));
    }

    #[test]
    fn measured_detail_region_owns_gap_and_footer_then_returns_to_two_regions() {
        fn measured(left: f32, top: f32, width: f32, height: f32) -> Option<BagInputRegion> {
            // The host must use actual ComputedNode/UiGlobalTransform geometry,
            // including backend DPR, rather than the intended Node pixel values.
            let node = ComputedNode { size: bevy::math::Vec2::new(width * 2.0, height * 2.0),
                inverse_scale_factor: 0.5, ..Default::default() };
            let transform = UiGlobalTransform::from_xy(
                (left + width * 0.5) * 2.0, (top + height * 0.5) * 2.0);
            node_region(&node, &transform)
        }
        let css_scale = 640.0 / 1368.0;
        let scale = 1.28 / css_scale;
        let left = (1368.0 - 448.0 * scale) * 0.5;
        let top = (768.0 - 236.0 * scale) * 0.5;
        let panel = measured(left, top, 316.0 * scale, 236.0 * scale).unwrap();
        let inspector = measured(left + 326.0 * scale, top, 122.0 * scale, 575.0).unwrap();
        let overlay = measured(left, top, 448.0 * scale, 236.0 * scale).unwrap();
        let gap = ((panel.right() + inspector.left) * 0.5, top + 100.0);
        let next = (left + (334.0 + 53.0) * scale, top + (196.0 + 16.0) * scale);
        assert!(!panel.contains(gap.0, gap.1) && !inspector.contains(gap.0, gap.1));
        assert!(!panel.contains(next.0, next.1) && !inspector.contains(next.0, next.1));
        let regions = published_input_regions(vec![panel], vec![inspector], vec![overlay],
            true, Some((1368.0, 768.0))).expect("measured detail covers gap and footer");
        assert_eq!(regions.len(), 3);
        assert!(regions.iter().any(|region| region.contains(gap.0, gap.1)));
        assert!(regions.iter().any(|region| region.contains(next.0, next.1)));
        assert!(published_input_regions(vec![panel], vec![inspector], Vec::new(),
            true, Some((1368.0, 768.0))).is_none(), "missing detail cannot be ready");
        assert!(published_input_regions(vec![panel], vec![inspector],
            measured(left, top, 0.0, 236.0 * scale).into_iter().collect(),
            true, Some((1368.0, 768.0))).is_none(), "unlaid-out detail cannot be ready");
        assert!(published_input_regions(vec![panel], vec![inspector],
            vec![measured(left, top, 448.0 * scale, 575.0).unwrap()],
            true, Some((1368.0, 768.0))).is_none(), "footer outside detail cannot be ready");
        let closed = published_input_regions(vec![panel], vec![inspector], Vec::new(),
            false, Some((1368.0, 768.0))).expect("closed detail restores the two regions");
        assert_eq!(closed, vec![panel, inspector]);
        assert!(!closed.iter().any(|region| region.contains(gap.0, gap.1)));
        assert!(!closed.iter().any(|region| region.contains(next.0, next.1)));
    }

    #[test]
    fn visible_icon_requires_authoritative_frame_and_touch_falls_back() {
        let mut snapshot = WebBagSnapshot::parse(&valid().to_string()).unwrap();
        snapshot.model.items[0].icon = 0;
        assert!(visible_icons(&snapshot.model, BagPage::Bag1).is_err());
        snapshot.model.items[0].icon = 1;
        assert_eq!(visible_icons(&snapshot.model, BagPage::Bag1).unwrap(), vec!["original-ui/Items/1.png"]);
        let metrics = snapshot.presentation.unwrap();
        assert!(metrics.matches_window(1024.0, 768.0));
        let mut canvas_resolution = bevy::window::WindowResolution::new(1024, 768);
        for backend_factor in [1.0, 2.0, 1.0] {
            canvas_resolution.set_scale_factor(backend_factor);
            canvas_resolution.set_physical_resolution(
                (1024.0 * backend_factor) as u32,
                (768.0 * backend_factor) as u32,
            );
            assert!(metrics.matches_window(canvas_resolution.width(), canvas_resolution.height()));
        }
        // A backend DPR without its corresponding physical resize is not a
        // valid shared layout. Keep the compatibility owner until it matches.
        canvas_resolution.set_scale_factor(2.0);
        assert!(!metrics.matches_window(canvas_resolution.width(), canvas_resolution.height()));
        assert!(!WebBagPresentation { touch: true, ..metrics }.matches_window(1024.0, 768.0));
    }

    #[test]
    fn touch_presentation_only_accepts_bounded_wide_landscape_with_matching_window() {
        for (logical_width, logical_height, stage_css_scale) in [
            (1368.0, 768.0, 640.0 / 1368.0),
            (1664.0, 768.0, 844.0 / 1664.0),
        ] {
            let metrics = WebBagPresentation {
                logical_width, logical_height, stage_css_scale, touch: true,
            };
            assert!(metrics.matches_window(logical_width, logical_height));
            assert!(!metrics.matches_window(logical_width - 2.0, logical_height));
        }
        for (width, height, scale) in [
            (1024.0, 768.0, 1.0),
            (1368.0, 768.0, 0.4),
            (768.0, 1368.0, 0.468),
        ] {
            assert!(!WebBagPresentation { logical_width: width, logical_height: height,
                stage_css_scale: scale, touch: true }.matches_window(width, height));
        }
    }

    fn with_belt_geometry() -> serde_json::Value {
        let mut snapshot = valid();
        snapshot["blockedUniqueIds"] = json!([]);
        snapshot["model"]["items"][0]["uniqueId"] = json!(7);
        snapshot["model"]["items"].as_array_mut().unwrap().push(json!({
            "uniqueId":0,"key":"belt","name":"Belt item","quantity":2,"slot":5,"container":1,
            "icon":2,"description":""
        }));
        snapshot["beltDropGeometry"] = json!({
            "revision":1,
            "targets":(0..6).map(|slot| json!({
                "slot":slot,"uniqueId":if slot == 5 { Some(0) } else { None },
                "left":300.0 + slot as f32 * 50.0,"top":100.0,"width":40.0,"height":40.0
            })).collect::<Vec<_>>()
        });
        snapshot
    }

    #[test]
    fn optional_belt_geometry_preserves_old_snapshot_and_exact_empty_or_zero_target() {
        let old = WebBagSnapshot::parse(&valid().to_string()).unwrap();
        assert!(old.belt_drop_geometry.is_none());
        let occupied = WebBagSnapshot::parse(&with_belt_geometry().to_string()).unwrap();
        assert_eq!(occupied.belt_drop_geometry.unwrap().targets[5].unique_id, Some(0));
        let mut empty = with_belt_geometry();
        empty["model"]["items"].as_array_mut().unwrap().pop();
        empty["beltDropGeometry"]["targets"][5]["uniqueId"] = serde_json::Value::Null;
        assert!(WebBagSnapshot::parse(&empty.to_string()).is_ok());
        let mut missing = empty;
        missing["beltDropGeometry"]["targets"][5].as_object_mut().unwrap().remove("uniqueId");
        assert!(WebBagSnapshot::parse(&missing.to_string()).is_err());
    }

    #[test]
    fn strict_belt_geometry_rejects_partial_ambiguous_unknown_and_unbounded_targets() {
        for case in ["five", "seven", "duplicate_slot", "slot6", "zero_revision", "unsafe_revision",
            "overlap", "zero_width", "negative_left", "outside", "unknown_identity", "wrong_identity",
            "missing_presentation", "occupied_as_empty", "duplicate_uid", "unsafe_uid", "unknown_field"] {
            let mut snapshot = with_belt_geometry();
            match case {
                "five" => { snapshot["beltDropGeometry"]["targets"].as_array_mut().unwrap().pop(); },
                "seven" => {
                    let target = snapshot["beltDropGeometry"]["targets"][0].clone();
                    snapshot["beltDropGeometry"]["targets"].as_array_mut().unwrap().push(target);
                },
                "duplicate_slot" => snapshot["beltDropGeometry"]["targets"][5]["slot"] = json!(0),
                "slot6" => snapshot["beltDropGeometry"]["targets"][5]["slot"] = json!(6),
                "zero_revision" => snapshot["beltDropGeometry"]["revision"] = json!(0),
                "unsafe_revision" => snapshot["beltDropGeometry"]["revision"] = json!(MAX_SAFE_JS_INTEGER + 1),
                "overlap" => snapshot["beltDropGeometry"]["targets"][5]["left"] = json!(300),
                "zero_width" => snapshot["beltDropGeometry"]["targets"][5]["width"] = json!(0),
                "negative_left" => snapshot["beltDropGeometry"]["targets"][5]["left"] = json!(-1),
                "outside" => snapshot["beltDropGeometry"]["targets"][5]["left"] = json!(1024),
                "unknown_identity" => snapshot["model"]["items"][1]["uniqueId"] = serde_json::Value::Null,
                "wrong_identity" => snapshot["beltDropGeometry"]["targets"][5]["uniqueId"] = json!(1),
                "missing_presentation" => snapshot["presentation"] = serde_json::Value::Null,
                "occupied_as_empty" => snapshot["beltDropGeometry"]["targets"][5]["uniqueId"] = serde_json::Value::Null,
                "duplicate_uid" => snapshot["model"]["items"][0]["uniqueId"] = json!(0),
                "unsafe_uid" => snapshot["beltDropGeometry"]["targets"][5]["uniqueId"] = json!(MAX_SAFE_JS_INTEGER + 1),
                "unknown_field" => snapshot["beltDropGeometry"]["targets"][5]["fabricated"] = json!(true),
                _ => unreachable!(),
            }
            assert!(WebBagSnapshot::parse(&snapshot.to_string()).is_err(), "{case}");
        }
        let mut snapshot = WebBagSnapshot::parse(&with_belt_geometry().to_string()).unwrap();
        snapshot.belt_drop_geometry.as_mut().unwrap().targets[0].left = f32::NAN;
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn bag_to_belt_capability_and_serialization_are_additive_to_abi_one() {
        let status = serde_json::to_value(WebBagStatus::default()).unwrap();
        assert_eq!(status["supportsBagToBelt"], true);
        assert!(status["appliedBeltGeometryRevision"].is_null());
        let source = mir2_client_bevy::bag_ui::BagSelection { container: 0, slot: 40, unique_id: 0 };
        let intent = BagUiIntent {
            identity: BagIdentity { run_generation: 1, connection_generation: 2,
                session_generation: 3, owner_revision: 4 },
            intent_sequence: 5, model_revision: 6, presentation_revision: 7,
            kind: BagUiIntentKind::MoveToBelt { source, target_slot: 5,
                target_unique_id: Some(8), belt_geometry_revision: 9 },
        };
        let value: serde_json::Value = serde_json::from_str(&encode_intent(intent)).unwrap();
        assert_eq!(value, json!({
            "runGeneration":1,"connectionGeneration":2,"sessionGeneration":3,"ownerRevision":4,
            "intentSequence":5,"modelRevision":6,"presentationRevision":7,
            "type":"moveToBelt","source":{"container":0,"slot":40,"uniqueId":0},
            "target":{"container":1,"slot":5,"uniqueId":8},"beltGeometryRevision":9
        }));
        let empty = BagUiIntent { kind: BagUiIntentKind::MoveToBelt { source,
            target_slot: 0, target_unique_id: None, belt_geometry_revision: 9 }, ..intent };
        let value: serde_json::Value = serde_json::from_str(&encode_intent(empty)).unwrap();
        assert!(value["target"]["uniqueId"].is_null());
        let legacy = BagUiIntent { kind: BagUiIntentKind::MoveItem { source, target_slot: 2 }, ..intent };
        let value: serde_json::Value = serde_json::from_str(&encode_intent(legacy)).unwrap();
        assert_eq!(value["type"], "moveItem");
        assert_eq!(value["target"], json!({"container":0,"slot":2}));
        assert!(value.get("beltGeometryRevision").is_none());
    }
}
