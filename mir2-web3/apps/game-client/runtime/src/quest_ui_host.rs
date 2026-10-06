//! Browser host boundary for the shared Bevy quest surface.
//!
//! This is a presentation input/output adapter. The browser owns the Gateway
//! socket, request-id allocator, authoritative snapshots, and ACK correlation.

use mir2_client_bevy::inventory::CrystalItemTooltipSourceModel;
use mir2_client_bevy::portable_quest_ui::QuestUiHostContext;
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::portable_hud_bar_draw_plan::applied_draw_plan;
use mir2_client_bevy::portable_hud_bar_draw_plan::HudBarDrawPlan;
use mir2_client_bevy::portable_hud_bar_draw_plan::HudBarPlanCommit;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::collections::HashSet;

#[cfg(target_arch = "wasm32")]
use bevy::asset::LoadState;
#[cfg(any(target_arch = "wasm32", test))]
use bevy::camera::{visibility::RenderLayers, ClearColorConfig, RenderTarget};
#[cfg(any(target_arch = "wasm32", test))]
use bevy::prelude::*;
#[cfg(target_arch = "wasm32")]
use bevy::text::Font;
#[cfg(any(target_arch = "wasm32", test))]
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};

#[cfg(target_arch = "wasm32")]
use js_sys::Function;
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::crystal_ui::{
    widget::{CrystalHintSet, CrystalHintSurface, Mir2CrystalHintPlugin},
    NativePlayerUiSet,
};
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::pending_operations::{
    apply_quest_operation_ack, PendingLifecycleSet, PendingOperations,
};
#[cfg(any(target_arch = "wasm32", test))]
use mir2_client_bevy::portable_experience_bar_ui::{
    ExperienceBarHostContext, ExperienceBarObservation, ExperienceBarObservationSet,
    ExperienceBarRect, Mir2PortableExperienceBarUiPlugin,
};
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::portable_hp_orb_ui::{
    HpOrbHostContext, HpOrbObservation, HpOrbObservationSet, HpOrbRect, HpOrbSlot,
    Mir2PortableHpOrbUiPlugin, MpOrbObservation,
};
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::portable_quest_ui::{
    Mir2PortableQuestUiPlugin, QuestUiFont, QuestUiTargetCamera,
    PORTABLE_QUEST_REQUIRED_SKINS,
};
#[cfg(any(target_arch = "wasm32", test))]
use mir2_client_bevy::portable_weight_bar_ui::{
    Mir2PortableWeightBarUiPlugin, WeightBarHostContext, WeightBarObservation,
    WeightBarObservationSet, WeightBarRect,
};
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::quest_guidance::QuestGuidance;
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::quest_intents::{QuestUiIntent, QuestUiIntentQueue};
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::quest_journey::NewcomerJourneyCatalog;
use mir2_client_bevy::quest_model::{
    CompletedQuestTracker, NpcDialogLine, NpcDialogModel, NpcDialogOption, Quest, QuestDetailText,
    QuestObjective, QuestReward, QuestStatus, QuestTracker,
};
use mir2_client_bevy::quest_ui::QuestUiPresentation;
use mir2_client_bevy::quest_presentation_text::QuestPresentationLocale;
#[cfg(target_arch = "wasm32")]
use mir2_client_bevy::quest_ui::{NpcDialogNav, QuestUiState};
use mir2_client_bevy::read_model::{PlayerStats, UiReadModel};
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebQuestUiSnapshot {
    pub generation: u64,
    pub revision: u64,
    #[serde(default)]
    pub language: QuestPresentationLocale,
    pub in_game: bool,
    pub host_visible: bool,
    pub quest_log_open: bool,
    pub open_revision: u64,
    pub blocks_gameplay_keys: bool,
    pub turn_in_blocked: bool,
    pub profile: String,
    pub quests: Vec<WebQuest>,
    pub completed_known: bool,
    pub completed_quest_ids: Vec<i32>,
    pub dialog: WebNpcDialog,
    pub player: Option<WebQuestPlayer>,
    #[serde(default)]
    pub presentation: Option<WebQuestUiPresentation>,
    #[serde(default)]
    pub hp_orb_slot: Option<WebHpOrbSlot>,
    #[serde(default)]
    pub experience_bar_slot: Option<WebExperienceBarRect>,
    #[serde(default)]
    pub weight_bar_slot: Option<WebExperienceBarRect>,
    #[serde(default)]
    pub hud_bar_plan: Option<HudBarPlanCommit>,
    #[serde(default)]
    pub world_context: Option<WebQuestWorldContext>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebQuestWorldContext {
    generation: u64,
    revision: u64,
    connection_generation: u64,
    session_generation: u64,
    scene_revision: u64,
    player_object_id: u32,
    map_file_name: String,
    #[serde(deserialize_with = "mir2_client_bevy::portable_quest_ui::required_nullable")]
    map_index: Option<i32>,
    entities: Vec<mir2_client_bevy::portable_quest_ui::QuestWorldEntity>,
    #[serde(deserialize_with = "mir2_client_bevy::portable_quest_ui::required_nullable")]
    selected_object_id: Option<u32>,
    ground_drops: Vec<mir2_client_bevy::portable_quest_ui::QuestGroundDrop>,
    inventory: WebQuestInventory,
    player: WebQuestSupplyPlayer,
    #[serde(default)]
    known_skills: Option<Vec<serde_json::Value>>,
}

/// Quest enables supply guidance only from a complete carried-item projection.
/// The generic inventory model's legacy capacity default is not evidence here.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebQuestInventory {
    capacity: u16,
    gold: u32,
    items: Vec<mir2_client_bevy::inventory::ItemModel>,
    #[serde(default)]
    npc_gold_trade_capacity: Option<mir2_client_bevy::inventory::NpcGoldTradeCapacity>,
}

impl WebQuestInventory {
    fn into_model(self) -> mir2_client_bevy::inventory::InventoryModel {
        mir2_client_bevy::inventory::InventoryModel {
            capacity: self.capacity,
            gold: self.gold,
            items: self.items,
            npc_gold_trade_capacity: self.npc_gold_trade_capacity,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebQuestSupplyPlayer {
    level: u32,
    class_name: String,
    gender: String,
    gold: u32,
    current_weight: u16,
    current_weight_known: bool,
    max_weight: u16,
    #[serde(default)]
    weights: Option<mir2_client_bevy::read_model::PlayerWeights>,
}

impl WebQuestWorldContext {
    fn validate(&self, generation: u64, revision: u64) -> bool {
        self.generation == generation && self.revision == revision
            && (1..=MAX_SAFE_JS_INTEGER).contains(&self.connection_generation)
            && (1..=MAX_SAFE_JS_INTEGER).contains(&self.session_generation)
            && (1..=MAX_SAFE_JS_INTEGER).contains(&self.scene_revision)
            && self.player_object_id > 0
            && !self.map_file_name.is_empty() && self.map_file_name.len() <= 128
            && !self.map_file_name.contains('\0')
            && self.map_file_name == self.map_file_name.trim()
                .strip_suffix(".map").unwrap_or(self.map_file_name.trim()).to_lowercase()
            && self.inventory.capacity == mir2_client_bevy::inventory::InventoryModel::canonical_capacity(self.inventory.capacity)
            && self.inventory.gold == self.player.gold && self.player.level > 0
            && !self.player.class_name.is_empty()
            && matches!(self.player.gender.as_str(), "male" | "female")
            && self.known_skills.as_ref().is_none_or(|rows| rows.len() <= 512)
            && self.map_index.is_none_or(|index| index >= 0)
            && self.selected_object_id.is_none_or(|id| id > 0)
            && self.entities.len() <= 512 && self.ground_drops.len() <= 512
            && self.valid_world_rows()
    }
    fn valid_world_rows(&self) -> bool {
        use mir2_client_bevy::entities::EntityKind;
        let text = |value: &str| value.len() <= 256 && !value.contains('\0');
        let mut ids = HashSet::new();
        let mut self_count = 0;
        for row in &self.entities {
            if row.object_id == 0 || !ids.insert(row.object_id) || !text(&row.name)
                || row.direction.as_ref().is_some_and(|value| value.len() > 32 || value.contains('\0')) {
                return false;
            }
            if row.kind == EntityKind::SelfPlayer {
                self_count += 1;
                if row.object_id != self.player_object_id { return false; }
            } else if row.object_id == self.player_object_id { return false; }
        }
        if self_count > 1 { return false; }
        self.ground_drops.iter().all(|row| row.object_id > 0 && ids.insert(row.object_id)
            && row.quantity > 0 && text(&row.name) && text(&row.source_monster))
    }
    fn into_resource(self) -> mir2_client_bevy::portable_quest_ui::QuestWorldContext {
        use mir2_client_bevy::portable_quest_ui::{QuestWorldContext, QuestWorldStamp};
        let player = PlayerStats {
            level: self.player.level,
            class_name: Some(self.player.class_name),
            gender: Some(self.player.gender),
            gold: self.player.gold,
            current_weight: self.player.current_weight,
            current_weight_known: self.player.current_weight_known,
            max_weight: self.player.max_weight,
            weights: self.player.weights,
            ..Default::default()
        };
        let skills = self.known_skills.as_deref()
            .and_then(|rows| mir2_client_bevy::skill_page_state::normalize_raw_skills(rows).ok());
        let mut context = QuestWorldContext { stamp: Some(QuestWorldStamp {
            generation: self.generation, revision: self.revision,
            connection_generation: self.connection_generation,
            session_generation: self.session_generation,
            scene_revision: self.scene_revision,
            player_object_id: self.player_object_id,
            map_file_name: self.map_file_name,
        }), inventory: Some(self.inventory.into_model()), player: Some(player), skills,
            map_index: self.map_index, raw_entities: self.entities,
            selected_object_id: self.selected_object_id, ground_drops: self.ground_drops,
            ..Default::default() };
        context.rebuild_projections();
        context
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebExperienceBarRect {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebHpOrbSlot {
    left: f32,
    top: f32,
}

/// CSS scale belongs to the browser host; layout dimensions must also agree
/// with the actual Bevy UI window before the shared surface can take input.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebQuestUiPresentation {
    logical_width: f32,
    logical_height: f32,
    stage_css_scale: f32,
    touch: bool,
}

impl WebQuestUiPresentation {
    fn matches_window(self, width: f32, height: f32) -> bool {
        [self.logical_width, self.logical_height, width, height]
            .into_iter()
            .all(|value| value.is_finite() && (1.0..=16_384.0).contains(&value))
            && self.stage_css_scale.is_finite()
            && self.stage_css_scale > 0.0
            && self.stage_css_scale <= 16.0
            && (self.logical_width - width).abs() <= 1.0
            && (self.logical_height - height).abs() <= 1.0
    }

    fn for_window(self, width: f32, height: f32) -> Option<QuestUiPresentation> {
        if !self.matches_window(width, height) {
            return None;
        }
        let presentation = QuestUiPresentation {
            logical_width: width,
            logical_height: height,
            stage_css_scale: self.stage_css_scale,
            touch: self.touch,
        };
        // Unsupported touch viewports retain the compatibility owner. They
        // must not silently inherit the small desktop font/button geometry.
        (!self.touch || presentation.supports_mobile_layout()).then_some(presentation)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebQuest {
    quest_index: i32,
    #[serde(default)]
    accept_npc_index: Option<u32>,
    #[serde(default)]
    finish_npc_index: Option<u32>,
    title: String,
    #[serde(default)]
    npc_name: Option<String>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    min_level_needed: i32,
    #[serde(default)]
    detail: WebQuestDetail,
    status: WebQuestStatus,
    #[serde(default)]
    objectives: Vec<WebQuestObjective>,
    #[serde(default)]
    rewards: Vec<WebQuestReward>,
    #[serde(default)]
    unknown_text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebQuestDetail {
    #[serde(default)]
    description_lines: Vec<String>,
    #[serde(default)]
    task_description_lines: Vec<String>,
    #[serde(default)]
    return_description_lines: Vec<String>,
    #[serde(default)]
    completion_description_lines: Vec<String>,
    #[serde(default)]
    time_limit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WebQuestStatus {
    NotStarted,
    InProgress,
    ReadyToTurnIn,
    Completed,
    Failed,
    Aborted,
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebQuestObjective {
    objective_id: String,
    text: String,
    current: u32,
    target: u32,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum WebQuestReward {
    Gold {
        amount: u32,
    },
    Experience {
        amount: u32,
    },
    Item {
        item_id: String,
        name: String,
        quantity: u32,
        #[serde(default)]
        icon: Option<u32>,
        #[serde(default)]
        selection_index: Option<i32>,
        #[serde(default)]
        tooltip_source: Option<CrystalItemTooltipSourceModel>,
    },
    Unknown {
        label: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebNpcDialog {
    pub is_open: bool,
    #[serde(default)]
    pub npc_object_id: Option<u32>,
    #[serde(default)]
    pub npc_name: Option<String>,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub options: Vec<WebNpcDialogOption>,
    pub has_input: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebNpcDialogOption {
    option_id: String,
    label: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WebQuestPlayer {
    hp: i32,
    max_hp: i32,
    #[serde(default)]
    mp: Option<i32>,
    #[serde(default)]
    max_mp: Option<i32>,
    level: u32,
    #[serde(default)]
    class_name: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    experience: Option<i64>,
    #[serde(default)]
    max_experience: Option<i64>,
    #[serde(default)]
    current_weight: Option<u16>,
    #[serde(default)]
    max_weight: Option<u16>,
}

pub(crate) struct QuestUiModels {
    pub quests: QuestTracker,
    pub completed: CompletedQuestTracker,
    pub dialog: NpcDialogModel,
    pub read_model: UiReadModel,
}

#[cfg(target_arch = "wasm32")]
const QUEST_FONT_ASSET: &str = "original-ui/fonts/NotoSansCJKsc-Regular.otf";

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
struct QuestUiHostAssets {
    font: Option<Handle<Font>>,
    skins: Vec<Handle<Image>>,
    requested: bool,
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
struct AppliedQuestUiSnapshot {
    generation: u64,
    revision: u64,
    language: QuestPresentationLocale,
    open_revision: u64,
    has_snapshot: bool,
    has_input: bool,
    in_game: bool,
    host_visible: bool,
    profile: String,
    presentation: Option<WebQuestUiPresentation>,
    presentation_ready: bool,
    hp_orb_slot: Option<WebHpOrbSlot>,
    player_known: bool,
    hp: i32,
    max_hp: i32,
    mp: Option<i32>,
    max_mp: Option<i32>,
    hp_only: bool,
    experience_bar_slot: Option<WebExperienceBarRect>,
    experience: Option<i64>,
    max_experience: Option<i64>,
    weight_bar_slot: Option<WebExperienceBarRect>,
    current_weight: Option<u16>,
    max_weight: Option<u16>,
    hud_bar_plan: Option<HudBarPlanCommit>,
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Resource)]
struct HudBarDrawPlanRoute(bool);

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Component)]
struct QuestUiHostWindow;

/// The feature may be present in the runtime even when this page did not opt in.
/// Avoid creating a second GPU surface (or requesting its assets) in that case.
#[cfg(target_arch = "wasm32")]
pub(crate) fn startup_mode() -> crate::webgl2_shared_ui::StartupUiMode {
    let global = js_sys::global();
    let Ok(location) = js_sys::Reflect::get(&global, &JsValue::from_str("location")) else {
        return crate::webgl2_shared_ui::startup_ui_mode(
            "",
            super::COMPILED_RENDER_BACKEND,
            true,
            cfg!(feature = "webgl2-shared-ui"),
        );
    };
    let Ok(search) = js_sys::Reflect::get(&location, &JsValue::from_str("search")) else {
        return crate::webgl2_shared_ui::startup_ui_mode(
            "",
            super::COMPILED_RENDER_BACKEND,
            true,
            cfg!(feature = "webgl2-shared-ui"),
        );
    };
    crate::webgl2_shared_ui::startup_ui_mode(
        search.as_string().as_deref().unwrap_or(""),
        super::COMPILED_RENDER_BACKEND,
        true,
        cfg!(feature = "webgl2-shared-ui"),
    )
}

/// Ordinary WebGL2 cannot present a second canvas from its first GLES device;
/// the fixed shared startup mode installs UI on the selected primary surface.
#[cfg(all(
    target_arch = "wasm32",
    feature = "webgl2",
    not(feature = "webgpu"),
    feature = "webgl2-shared-ui"
))]
pub(crate) fn install(app: &mut App, shared_webgl2: bool) {
    if shared_webgl2 {
        install_supported(app, true);
    } else {
        app.add_systems(Update, publish_webgl2_unsupported_status);
        crate::bag_ui_host::install_unsupported(app);
    }
}

#[cfg(all(
    target_arch = "wasm32",
    feature = "webgl2",
    not(feature = "webgpu"),
    not(feature = "webgl2-shared-ui")
))]
pub(crate) fn install(app: &mut App, _shared_webgl2: bool) {
    app.add_systems(Update, publish_webgl2_unsupported_status);
    crate::bag_ui_host::install_unsupported(app);
}

#[cfg(all(target_arch = "wasm32", feature = "webgl2", not(feature = "webgpu")))]
fn publish_webgl2_unsupported_status() {
    let snapshot = PENDING_SNAPSHOT.with(|mailbox| mailbox.borrow_mut().take());
    let Some(snapshot) = snapshot else {
        return;
    };
    STATUS.with(|status| {
        let mut status = status.borrow_mut();
        status.generation = snapshot.generation;
        status.revision = snapshot.revision;
        status.language = snapshot.language;
        status.open_revision = snapshot.open_revision;
        status.quest_log_open = snapshot.quest_log_open;
        status.ready = false;
        status.captures_pointer = false;
        status.hp_orb = None;
        status.mp_orb = None;
        status.experience_bar = None;
        status.weight_bar = None;
        status.hud_bar_draw_plan = None;
        status.error = Some(
            "Shared Quest UI and hints are unavailable in this WebGL2 runtime; React quest UI remains active".to_owned(),
        );
    });
    PENDING_ACKS.with(|mailbox| mailbox.borrow_mut().clear());
}

/// Install the portable UI into the existing world Bevy App. WebGPU can
/// present the same device to a sibling transparent canvas without a second
/// App or WASM instance.
#[cfg(all(
    target_arch = "wasm32",
    not(all(feature = "webgl2", not(feature = "webgpu")))
))]
pub(crate) fn install(app: &mut App, shared_webgl2: bool) {
    debug_assert!(!shared_webgl2);
    install_supported(app, false);
}

#[cfg(any(target_arch = "wasm32", test))]
fn ui_target_window(app: &mut App, shared_webgl2: bool) -> Entity {
    if shared_webgl2 {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<PrimaryWindow>>();
        let primary = query
            .single(world)
            .expect("single-surface UI requires the existing primary Window");
        world.entity_mut(primary).insert(QuestUiHostWindow);
        primary
    } else {
        app.world_mut()
            .spawn((
                Window {
                    title: "Mir2 Quest UI".to_owned(),
                    canvas: Some("#mir2-quest-ui-canvas".to_owned()),
                    transparent: true,
                    composite_alpha_mode: super::WINDOW_COMPOSITE_ALPHA_MODE,
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    // Winit owns physical pixels and input DPI together.
                    resolution: WindowResolution::new(1024, 768),
                    ..Default::default()
                },
                QuestUiHostWindow,
            ))
            .id()
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn ui_target_camera(app: &mut App, ui_window: Entity, shared_webgl2: bool) -> Entity {
    app.world_mut()
        .spawn((
            Camera2d,
            Camera {
                is_active: true,
                order: if shared_webgl2 { 1 } else { 0 },
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..Default::default()
            },
            RenderLayers::layer(30),
            RenderTarget::Window(WindowRef::Entity(ui_window)),
        ))
        .id()
}

#[cfg(all(
    target_arch = "wasm32",
    not(all(
        feature = "webgl2",
        not(feature = "webgpu"),
        not(feature = "webgl2-shared-ui")
    ))
))]
fn install_supported(app: &mut App, shared_webgl2: bool) {
    let ui_window = ui_target_window(app, shared_webgl2);
    let ui_camera = ui_target_camera(app, ui_window, shared_webgl2);
    app.insert_resource(QuestUiTargetCamera(ui_camera));
    app.insert_resource(CrystalHintSurface {
        camera: ui_camera,
        window: ui_window,
        font: Handle::default(),
        active: false,
    });
    app.insert_resource(QuestUiPresentation {
        logical_width: 1024.0,
        logical_height: 768.0,
        stage_css_scale: 1.0,
        touch: false,
    });
    // Feature availability alone never fetches the 16 MB CJK font. The first
    // explicit browser opt-in snapshot requests all UI assets together.
    app.insert_resource(QuestUiFont(Handle::default()));
    app.insert_resource(QuestUiHostAssets {
        font: None,
        skins: Vec::new(),
        requested: false,
    });
    app.init_resource::<AppliedQuestUiSnapshot>();
    app.add_plugins(Mir2PortableQuestUiPlugin);
    app.add_plugins(Mir2PortableHpOrbUiPlugin);
    crate::hud_ui_host::install(app, ui_window, ui_camera);
    install_bar_route(app, true);
    app.add_plugins(Mir2CrystalHintPlugin);
    crate::bag_ui_host::install(app, ui_window, ui_camera);
    crate::character_ui_host::install(app);
    crate::spells_ui_host::install(app);
    crate::mail_ui_host::install(app);
    app.configure_sets(
        Update,
        CrystalHintSet::Sync.after(PendingLifecycleSet::Ingest),
    );
    app.add_systems(
        Update,
        sync_quest_hint_surface
            .after(NativePlayerUiSet::Read)
            .before(CrystalHintSet::Sync),
    );
    app.add_systems(
        Update,
        (
            ingest_quest_ui_snapshot,
            sync_quest_ui_presentation,
            ingest_quest_ui_acks,
            sync_quest_ui_visibility,
        )
            .chain()
            .in_set(PendingLifecycleSet::Ingest).in_set(QuestHostIngestSet),
    );
    app.add_systems(
        PostUpdate,
        publish_quest_ui_status
            .after(mir2_client_bevy::quest_ui::QuestCompactObservationSet::Observe)
            .after(mir2_client_bevy::portable_quest_ui::QuestWorldControlObservationSet::Observe)
            .after(HpOrbObservationSet::Observe)
            .after(ExperienceBarObservationSet::Observe)
            .after(WeightBarObservationSet::Observe),
    );
    app.add_systems(Last, (forward_quest_ui_intents, forward_quest_presentation_actions));
}

/// Called before Startup, so the new route never creates painter roots or private leases.
#[cfg(any(target_arch = "wasm32", test))]
fn install_bar_route(app: &mut App, draw_plan: bool) {
    app.insert_resource(HudBarDrawPlanRoute(draw_plan));
    if draw_plan {
        app.init_resource::<ExperienceBarHostContext>()
            .init_resource::<ExperienceBarObservation>()
            .init_resource::<WeightBarHostContext>()
            .init_resource::<WeightBarObservation>();
    } else {
        app.add_plugins(Mir2PortableExperienceBarUiPlugin);
        app.add_plugins(Mir2PortableWeightBarUiPlugin);
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(SystemSet,Debug,Hash,PartialEq,Eq,Clone)]
pub(crate) struct QuestHostIngestSet;

#[cfg(target_arch = "wasm32")]
#[allow(clippy::too_many_arguments)]
fn ingest_quest_ui_snapshot(
    (mut applied, mut locale): (ResMut<AppliedQuestUiSnapshot>, ResMut<QuestPresentationLocale>),
    mut context: ResMut<QuestUiHostContext>,
    mut tracker: ResMut<QuestTracker>,
    mut completed: ResMut<CompletedQuestTracker>,
    mut dialog: ResMut<NpcDialogModel>,
    read_authority: (ResMut<UiReadModel>, ResMut<mir2_client_bevy::read_model::UiReadModelIngress>),
    mut guidance: ResMut<QuestGuidance>,
    mut catalog: ResMut<NewcomerJourneyCatalog>,
    mut pending: ResMut<PendingOperations>,
    mut intents: ResMut<QuestUiIntentQueue>,
    quest_interaction: (
        ResMut<QuestUiState>,
        ResMut<mir2_client_bevy::portable_quest_ui::QuestWorldContext>,
        ResMut<mir2_client_bevy::portable_quest_ui::QuestPresentationActionQueue>,
        ResMut<mir2_client_bevy::quest_ui::QuestRouteNavigationIntentQueue>,
        ResMut<mir2_client_bevy::portable_quest_ui::QuestRouteWorldStamp>,
    ),
    mut dialog_nav: ResMut<NpcDialogNav>,
    mut player_ui: ResMut<NativePlayerUiState>,
    asset_server: Res<AssetServer>,
    mut assets: ResMut<QuestUiHostAssets>,
    mut packaged_font: ResMut<QuestUiFont>,
) {
    let (mut read_model,mut read_ingress)=read_authority;
    let (mut quest_state, mut world_context, mut presentation_actions, mut routes, mut route_stamp) = quest_interaction;
    let Some(mut snapshot) = PENDING_SNAPSHOT.with(|mailbox| mailbox.borrow_mut().take()) else {
        return;
    };
    if applied.has_snapshot && !snapshot.is_newer_than(applied.generation, applied.revision)
    {
        return;
    }
    let new_generation = !applied.has_snapshot || snapshot.generation > applied.generation;
    // A hidden or changed scene cannot replay a local panel action on return.
    let next_stamp = snapshot.world_context.as_ref().map(|context| (
        context.generation, context.revision, context.connection_generation,
        context.session_generation, context.scene_revision, context.player_object_id,
        context.map_file_name.as_str()));
    let previous_stamp = world_context.stamp.as_ref().map(|stamp| (
        stamp.generation, stamp.revision, stamp.connection_generation,
        stamp.session_generation, stamp.scene_revision, stamp.player_object_id,
        stamp.map_file_name.as_str()));
    if new_generation || !snapshot.in_game || !snapshot.host_visible || next_stamp != previous_stamp {
        presentation_actions.clear();
        routes.clear();
        route_stamp.0 = None;
    }
    world_context.clear();
    if snapshot.in_game && snapshot.host_visible {
        if let Some(context) = snapshot.world_context.take() {
            *world_context = context.into_resource();
        }
    }
    if *locale != snapshot.language { *locale = snapshot.language; }
    applied.language = snapshot.language;
    if new_generation {
        pending.release_all_quest_operations();
        intents.clear();
        quest_state.reset();
        dialog_nav.clear();
        *player_ui = NativePlayerUiState::default();
        STATUS.with(|status| status.borrow_mut().error = None);
    }
    if new_generation || applied.profile != snapshot.profile {
        *guidance = QuestGuidance::from_profile_name(&snapshot.profile);
        *catalog = NewcomerJourneyCatalog::from_guidance(&guidance);
    }
    if snapshot.host_visible && !assets.requested {
        let font = asset_server.load::<Font>(QUEST_FONT_ASSET);
        assets.skins = PORTABLE_QUEST_REQUIRED_SKINS
            .iter()
            .map(|path| asset_server.load::<Image>(*path))
            .collect();
        packaged_font.0 = font.clone();
        assets.font = Some(font);
        assets.requested = true;
    }
    // The host's open/close revision changes only for an explicit external
    // request. Ordinary authoritative snapshots may not clobber Bevy clicks.
    if new_generation || snapshot.open_revision > applied.open_revision {
        context.quest_log_open = snapshot.quest_log_open;
        context.open_revision = snapshot.open_revision;
        applied.open_revision = snapshot.open_revision;
    }
    // The visual tree stays hidden until font, skins, snapshot and transport
    // are ready. `sync_quest_ui_visibility` applies that handoff each frame.
    context.generation = snapshot.generation;
    context.revision = snapshot.revision;
    context.host_visible = false;
    context.in_game = snapshot.in_game;
    context.blocks_gameplay_keys = snapshot.blocks_gameplay_keys;
    context.turn_in_blocked = snapshot.turn_in_blocked;

    let has_input = snapshot.dialog.has_input;
    let generation = snapshot.generation;
    let revision = snapshot.revision;
    let in_game = snapshot.in_game;
    let host_visible = snapshot.host_visible;
    let profile = snapshot.profile.clone();
    let presentation = snapshot.presentation;
    let hp_orb_slot = snapshot.hp_orb_slot;
    let experience_bar_slot = snapshot.experience_bar_slot;
    let weight_bar_slot = snapshot.weight_bar_slot;
    let hud_bar_plan = snapshot.hud_bar_plan.clone();
    let snapshot_experience = snapshot
        .player
        .as_ref()
        .and_then(|player| player.experience);
    let snapshot_max_experience = snapshot
        .player
        .as_ref()
        .and_then(|player| player.max_experience);
    let snapshot_current_weight = snapshot
        .player
        .as_ref()
        .and_then(|player| player.current_weight);
    let snapshot_max_weight = snapshot
        .player
        .as_ref()
        .and_then(|player| player.max_weight);
    let player_known = snapshot.player.is_some();
    let snapshot_mp = snapshot.player.as_ref().and_then(|player| player.mp);
    let snapshot_max_mp = snapshot.player.as_ref().and_then(|player| player.max_mp);
    let models = snapshot.into_models();
    let snapshot_hp = models.read_model.player.hp;
    let snapshot_max_hp = models.read_model.player.max_hp;
    let snapshot_hp_only =
        mir2_client_bevy::crystal_ui::hud_orb::crystal_hp_only(&models.read_model);
    *tracker = models.quests;
    *completed = models.completed;
    *dialog = models.dialog;
    read_ingress.apply_legacy_quest(&mut read_model, generation, revision, models.read_model.player);

    applied.generation = generation;
    applied.revision = revision;
    applied.has_snapshot = true;
    applied.has_input = has_input;
    applied.in_game = in_game;
    applied.host_visible = host_visible;
    applied.profile = profile;
    applied.presentation = presentation;
    applied.hp_orb_slot = hp_orb_slot;
    applied.player_known = player_known;
    applied.hp = snapshot_hp;
    applied.max_hp = snapshot_max_hp;
    applied.mp = snapshot_mp;
    applied.max_mp = snapshot_max_mp;
    applied.hp_only = snapshot_hp_only;
    applied.experience_bar_slot = experience_bar_slot;
    applied.experience = snapshot_experience;
    applied.max_experience = snapshot_max_experience;
    applied.weight_bar_slot = weight_bar_slot;
    applied.current_weight = snapshot_current_weight;
    applied.max_weight = snapshot_max_weight;
    applied.hud_bar_plan = hud_bar_plan;
}

#[cfg(target_arch = "wasm32")]
fn sync_quest_ui_presentation(
    mut applied: ResMut<AppliedQuestUiSnapshot>,
    mut hp_orb: ResMut<HpOrbHostContext>,
    mut experience_bar: ResMut<ExperienceBarHostContext>,
    mut weight_bar: ResMut<WeightBarHostContext>,
    windows: Query<&Window, With<QuestUiHostWindow>>,
    mut presentation: ResMut<QuestUiPresentation>,
) {
    let next = applied
        .presentation
        .zip(windows.single().ok())
        .and_then(|(metrics, window)| metrics.for_window(window.width(), window.height()));
    applied.presentation_ready = next.is_some();
    hp_orb.generation = applied.generation;
    hp_orb.revision = applied.revision;
    hp_orb.in_game = applied.in_game;
    hp_orb.player_known = applied.player_known;
    hp_orb.hp = applied.hp;
    hp_orb.max_hp = applied.max_hp;
    hp_orb.mp = applied.mp;
    hp_orb.max_mp = applied.max_mp;
    hp_orb.hp_only = applied.hp_only;
    hp_orb.slot = applied.hp_orb_slot.map(|slot| HpOrbSlot {
        left: slot.left,
        top: slot.top,
    });
    hp_orb.logical_size = applied
        .presentation
        .map(|metrics| Vec2::new(metrics.logical_width, metrics.logical_height));
    hp_orb.window_matches = applied
        .presentation
        .zip(windows.single().ok())
        .is_some_and(|(metrics, window)| metrics.matches_window(window.width(), window.height()));
    experience_bar.generation = applied.generation;
    experience_bar.revision = applied.revision;
    experience_bar.in_game = applied.in_game;
    // The HUD image can paint while the Diary font/panel is still preparing,
    // but an explicitly hidden host cannot transfer its measured slot.
    experience_bar.host_visible = applied.host_visible && applied.experience_bar_slot.is_some();
    experience_bar.experience = applied.experience;
    experience_bar.max_experience = applied.max_experience;
    experience_bar.slot = applied.experience_bar_slot.map(|slot| ExperienceBarRect {
        left: slot.left,
        top: slot.top,
        width: slot.width,
        height: slot.height,
    });
    experience_bar.logical_size = applied
        .presentation
        .map(|metrics| Vec2::new(metrics.logical_width, metrics.logical_height));
    experience_bar.window_matches = hp_orb.window_matches;
    weight_bar.generation = applied.generation;
    weight_bar.revision = applied.revision;
    weight_bar.in_game = applied.in_game;
    weight_bar.host_visible = applied.host_visible && applied.weight_bar_slot.is_some();
    weight_bar.current_weight = applied.current_weight;
    weight_bar.max_weight = applied.max_weight;
    weight_bar.slot = applied.weight_bar_slot.map(|slot| WeightBarRect {
        left: slot.left,
        top: slot.top,
        width: slot.width,
        height: slot.height,
    });
    weight_bar.logical_size = experience_bar.logical_size;
    weight_bar.window_matches = hp_orb.window_matches;
    if let Some(next) = next {
        if *presentation != next {
            *presentation = next;
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_quest_ui_visibility(
    applied: Res<AppliedQuestUiSnapshot>,
    mut context: ResMut<QuestUiHostContext>,
) {
    let ready = STATUS.with(|status| status.borrow().ready);
    context.layout_preparing = applied.has_snapshot
        && applied.presentation_ready
        && applied.host_visible
        && applied.in_game
        && !applied.has_input
        && applied.presentation.is_some_and(|metrics| {
            metrics.touch && metrics.logical_height * metrics.stage_css_scale < 359.0
        });
    context.host_visible = ready
        && applied.presentation_ready
        && applied.host_visible
        && applied.in_game
        && !applied.has_input;
}

#[cfg(target_arch = "wasm32")]
fn quest_ui_has_open_surface(
    player_ui: &NativePlayerUiState,
    quest_state: &QuestUiState,
    dialog: &NpcDialogModel,
) -> bool {
    player_ui.quest_open()
        || quest_state.detail_quest_index.is_some()
        || quest_state.npc_quest_list_open
        || quest_state.abandon_confirmation_quest_index.is_some()
        || quest_state.quest_alert_message.is_some()
        || dialog.is_open
}

/// Hints use the same window, packaged font and current ownership as the
/// shared Quest surface. Apply after its UI mutations so closing a panel or
/// yielding ownership cannot leave a tooltip from the previous frame visible.
#[cfg(target_arch = "wasm32")]
fn sync_quest_hint_surface(
    context: Res<QuestUiHostContext>,
    player_ui: Res<NativePlayerUiState>,
    quest_state: Res<QuestUiState>,
    dialog: Res<NpcDialogModel>,
    font: Res<QuestUiFont>,
    bag: Res<mir2_client_bevy::portable_bag_ui::BagUiHostContext>,
    mut surface: ResMut<CrystalHintSurface>,
) {
    let active = (context.in_game
        && context.host_visible
        && quest_ui_has_open_surface(&player_ui, &quest_state, &dialog))
        || (bag.bag_open && bag.input_enabled && bag.presentation_ready);
    if surface.active != active {
        surface.active = active;
    }
    if surface.font != font.0 {
        surface.font = font.0.clone();
    }
}

#[cfg(target_arch = "wasm32")]
fn ingest_quest_ui_acks(
    applied: Res<AppliedQuestUiSnapshot>,
    mut pending: ResMut<PendingOperations>,
) {
    let acks = PENDING_ACKS.with(|mailbox| std::mem::take(&mut *mailbox.borrow_mut()));
    for ack in acks {
        if !applied.has_snapshot || ack.generation != applied.generation {
            continue;
        }
        if let Some(ack) = ack.into_shared() {
            // ACK and NACK release the exact business key plus request ID. They
            // never mutate authoritative quest status or grant a reward.
            apply_quest_operation_ack(&mut pending, &ack);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn publish_quest_ui_status(
    applied: Res<AppliedQuestUiSnapshot>,
    plan_route: Res<HudBarDrawPlanRoute>,
    hp_orb: Res<HpOrbObservation>,
    mp_orb: Res<MpOrbObservation>,
    bar_inputs: (
        Res<ExperienceBarObservation>,
        Res<WeightBarObservation>,
        Res<ExperienceBarHostContext>,
        Res<WeightBarHostContext>,
    ),
    assets: Res<QuestUiHostAssets>,
    server: Res<AssetServer>,
    roots: Query<Entity, With<mir2_client_bevy::quest_ui::QuestUiRoot>>,
    quest_world: (Res<QuestUiHostContext>, Res<mir2_client_bevy::portable_quest_ui::QuestWorldContext>,
        Res<mir2_client_bevy::portable_quest_ui::QuestWorldControlRects>, Res<QuestTracker>),
    player_ui: Res<NativePlayerUiState>,
    quest_state: Res<QuestUiState>,
    dialog: Res<NpcDialogModel>,
    compact: Res<mir2_client_bevy::quest_ui::QuestCompactReadiness>,
    model: Res<UiReadModel>,
) {
    let (context, world, rects, tracker) = quest_world;
    NAME_TARGETS.with(|published| {
        *published.borrow_mut() = QuestNameTargets::from_context(&world, &context, &tracker,
            applied.has_snapshot && applied.host_visible && applied.in_game
                && world.stamp.as_ref().is_some_and(|stamp| stamp.generation == applied.generation && stamp.revision == applied.revision));
    });
    WORLD_CONTROL_RECTS.with(|published| {
        *published.borrow_mut() = if world.current(&context) && rects.stamp == world.stamp
            && applied.has_snapshot && applied.presentation_ready && applied.host_visible && applied.in_game
            && rects.stamp.as_ref().is_some_and(|stamp| stamp.generation == applied.generation && stamp.revision == applied.revision) {
            (*rects).clone()
        } else { mir2_client_bevy::portable_quest_ui::QuestWorldControlRects::default() };
    });
    let (experience_bar, weight_bar, experience_bar_context, weight_bar_context) = bar_inputs;
    let asset_state = assets
        .font
        .iter()
        .map(|font| server.get_load_state(font.id()))
        .chain(
            assets
                .skins
                .iter()
                .map(|handle| server.get_load_state(handle.id())),
        );
    let mut loading = !assets.requested;
    let mut asset_error = false;
    for state in asset_state {
        match state {
            Some(LoadState::Loaded) => {}
            Some(LoadState::Failed(_)) => asset_error = true,
            _ => loading = true,
        }
    }
    STATUS.with(|status| {
        let mut status = status.borrow_mut();
        // This counter belongs to the WASM instance, not a character session.
        // A stopped Bevy update loop leaves it unchanged so the browser can
        // hand the Quest surface back to React instead of trusting stale ready.
        status.frame = status.frame.saturating_add(1).min(MAX_SAFE_JS_INTEGER);
        status.generation = applied.generation;
        status.revision = applied.revision;
        status.language = applied.language;
        status.open_revision = context.open_revision;
        status.quest_log_open = player_ui.quest_open();
        let hp_ready = hp_orb.ready
            && applied.has_snapshot
            && applied.in_game
            && hp_orb.generation == applied.generation
            && hp_orb.revision == applied.revision
            && hp_orb.slot.is_some()
            && hp_orb.image.is_some()
            && hp_orb.source.is_some()
            && hp_orb.destination.is_some()
            && hp_orb.layout.is_some();
        status.hp_orb = Some(WebHpOrbStatus {
            supported: true,
            ready: hp_ready,
            generation: applied.generation,
            revision: applied.revision,
            hp: hp_ready.then_some(hp_orb.hp),
            max_hp: hp_ready.then_some(hp_orb.max_hp),
            hp_only: hp_ready.then_some(hp_orb.hp_only),
            slot: hp_ready.then(|| WebHpOrbSlot {
                left: hp_orb.slot.expect("ready orb has slot").left,
                top: hp_orb.slot.expect("ready orb has slot").top,
            }),
            image: hp_ready.then(|| hp_orb.image.expect("ready orb has image")),
            source: hp_ready
                .then(|| WebHpOrbRect::from(hp_orb.source.expect("ready orb has source"))),
            destination: hp_ready.then(|| {
                WebHpOrbRect::from(hp_orb.destination.expect("ready orb has destination"))
            }),
            layout: hp_ready
                .then(|| WebHpOrbRect::from(hp_orb.layout.expect("ready orb has layout"))),
        });
        let mp_ready = mp_orb.ready
            && applied.has_snapshot
            && applied.in_game
            && mp_orb.generation == applied.generation
            && mp_orb.revision == applied.revision
            && mp_orb.slot.is_some()
            && mp_orb.image.is_some()
            && mp_orb.source.is_some()
            && mp_orb.destination.is_some()
            && mp_orb.layout.is_some();
        status.mp_orb = Some(WebMpOrbStatus {
            supported: true,
            ready: mp_ready,
            generation: applied.generation,
            revision: applied.revision,
            mp: mp_ready.then_some(mp_orb.mp),
            max_mp: mp_ready.then_some(mp_orb.max_mp),
            hp_only: mp_ready.then_some(false),
            slot: mp_ready.then(|| WebHpOrbSlot {
                left: mp_orb.slot.expect("ready MP orb has slot").left,
                top: mp_orb.slot.expect("ready MP orb has slot").top,
            }),
            image: mp_ready.then(|| mp_orb.image.expect("ready MP orb has image")),
            source: mp_ready
                .then(|| WebHpOrbRect::from(mp_orb.source.expect("ready MP orb has source"))),
            destination: mp_ready.then(|| {
                WebHpOrbRect::from(mp_orb.destination.expect("ready MP orb has destination"))
            }),
            layout: mp_ready
                .then(|| WebHpOrbRect::from(mp_orb.layout.expect("ready MP orb has layout"))),
        });
        let experience_ready = !plan_route.0
            && experience_bar.ready
            && applied.has_snapshot
            && applied.in_game
            && experience_bar.generation == applied.generation
            && experience_bar.revision == applied.revision
            && experience_bar.slot.is_some()
            && experience_bar.image.is_some()
            && experience_bar.source.is_some()
            && experience_bar.destination.is_some()
            && experience_bar.layout.is_some();
        status.experience_bar = Some(WebExperienceBarStatus {
            supported: !plan_route.0,
            ready: experience_ready,
            generation: applied.generation,
            revision: applied.revision,
            experience: experience_ready.then_some(experience_bar.experience),
            max_experience: experience_ready.then_some(experience_bar.max_experience),
            slot: experience_ready.then(|| {
                WebExperienceBarRect::from(experience_bar.slot.expect("ready bar has slot"))
            }),
            image: experience_ready.then(|| experience_bar.image.expect("ready bar has image")),
            source: experience_ready.then(|| {
                WebExperienceBarRect::from(experience_bar.source.expect("ready bar has source"))
            }),
            destination: experience_ready.then(|| {
                WebExperienceBarRect::from(
                    experience_bar
                        .destination
                        .expect("ready bar has destination"),
                )
            }),
            layout: experience_ready.then(|| {
                WebExperienceBarRect::from(experience_bar.layout.expect("ready bar has layout"))
            }),
        });
        let weight_ready = !plan_route.0
            && weight_bar.ready
            && applied.has_snapshot
            && applied.in_game
            && weight_bar.generation == applied.generation
            && weight_bar.revision == applied.revision
            && weight_bar.slot.is_some()
            && weight_bar.image.is_some()
            && weight_bar.source.is_some()
            && weight_bar.destination.is_some()
            && weight_bar.layout.is_some();
        status.weight_bar = Some(WebWeightBarStatus {
            supported: !plan_route.0,
            ready: weight_ready,
            generation: applied.generation,
            revision: applied.revision,
            current_weight: weight_ready.then_some(weight_bar.current_weight),
            max_weight: weight_ready.then_some(weight_bar.max_weight),
            slot: weight_ready
                .then(|| WebExperienceBarRect::from(weight_bar.slot.expect("ready weight slot"))),
            image: weight_ready.then(|| weight_bar.image.expect("ready weight image")),
            source: weight_ready.then(|| {
                WebExperienceBarRect::from(weight_bar.source.expect("ready weight source"))
            }),
            destination: weight_ready.then(|| {
                WebExperienceBarRect::from(
                    weight_bar.destination.expect("ready weight destination"),
                )
            }),
            layout: weight_ready.then(|| {
                WebExperienceBarRect::from(weight_bar.layout.expect("ready weight layout"))
            }),
        });
        status.hud_bar_draw_plan = if plan_route.0 && applied.has_snapshot {
            applied.hud_bar_plan.as_ref().and_then(|commit| {
                applied_draw_plan(
                    applied.generation,
                    applied.revision,
                    commit,
                    &experience_bar_context,
                    &weight_bar_context,
                    &model,
                )
            })
        } else {
            None
        };
        status.compact = context.layout_preparing.then(|| WebCompactQuestStatus {
            current: compact.current,
            generation: compact.generation,
            revision: compact.revision,
            sheet: compact.sheet,
            page: compact.page,
            quest_index: compact.quest_index,
            reward_page: compact.reward_page,
            text_top: compact.text_top,
            diary_page: compact.diary_page,
            action_count: compact.action_count,
            text_count: compact.text_count,
            min_css_hit_width: compact.min_css_hit_width,
            min_css_hit_height: compact.min_css_hit_height,
            min_css_text_size: compact.min_css_text_size,
            panel_css_bounds: compact
                .panel_css_bounds
                .map(|rect| WebCompactQuestPanelRect {
                    left: rect.left,
                    top: rect.top,
                    width: rect.width,
                    height: rect.height,
                }),
            first_failure: compact.first_failure,
            action_bounds: compact
                .actions
                .iter()
                .take(24)
                .map(|rect| WebCompactQuestActionRect {
                    label: rect.label,
                    left: rect.left,
                    top: rect.top,
                    width: rect.width,
                    height: rect.height,
                })
                .collect(),
        });
        if asset_error {
            status.error = Some("quest UI font or skin failed to load".to_owned());
        }
        const PRESENTATION_ERROR: &str = "Quest UI viewport metrics are not ready";
        if applied.presentation_ready && status.error.as_deref() == Some(PRESENTATION_ERROR) {
            status.error = None;
        } else if applied.has_snapshot && !applied.presentation_ready && status.error.is_none() {
            status.error = Some(PRESENTATION_ERROR.to_owned());
        }
        let can_present = applied.has_snapshot
            && applied.presentation_ready
            && applied.in_game
            && applied.host_visible
            && !applied.has_input
            && !loading
            && !asset_error
            && roots.single().is_ok()
            && (!context.layout_preparing || compact.current)
            && INTENT_SINK.with(|sink| sink.borrow().is_some());
        status.ready = can_present && status.error.is_none();
        status.captures_pointer = status.ready
            && context.host_visible
            && quest_ui_has_open_surface(&player_ui, &quest_state, &dialog);
        if applied.has_input && dialog.is_open {
            status.error =
                Some("NPC text input is not yet supported by shared Quest UI".to_owned());
            status.ready = false;
            status.captures_pointer = false;
        } else if status.error.as_deref()
            == Some("NPC text input is not yet supported by shared Quest UI")
        {
            status.error = None;
        }
    });
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebIntentResult {
    accepted: bool,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[cfg(target_arch = "wasm32")]
fn encode_quest_intent(intent: &QuestUiIntent, generation: u64) -> Result<String, &'static str> {
    use serde_json::json;
    let message = match intent {
        QuestUiIntent::AcceptQuest {
            npc_index,
            quest_index,
        } => json!({
            "generation": generation, "type": "acceptQuest",
            "npcIndex": npc_index, "questIndex": quest_index,
        }),
        QuestUiIntent::FinishQuest {
            quest_index,
            selected_item_index,
        } => json!({
            "generation": generation, "type": "finishQuest",
            "questIndex": quest_index, "selectedItemIndex": selected_item_index,
        }),
        QuestUiIntent::AbandonQuest { quest_index } => json!({
            "generation": generation, "type": "abandonQuest", "questIndex": quest_index,
        }),
        QuestUiIntent::ShareQuest { quest_index } => json!({
            "generation": generation, "type": "shareQuest", "questIndex": quest_index,
        }),
        QuestUiIntent::SelectNpcDialog { target } => json!({
            "generation": generation, "type": "selectNpcDialog", "target": target,
        }),
        QuestUiIntent::InteractQuestNpc { .. }
        | QuestUiIntent::InteractNpc { .. }
        | QuestUiIntent::AttackTarget { .. }
        | QuestUiIntent::HarvestDirection { .. }
        | QuestUiIntent::PickUpObject { .. }
        | QuestUiIntent::PickUpTile => {
            return Err("unsupported shared Quest UI intent");
        }
    };
    serde_json::to_string(&message).map_err(|_| "could not encode Quest UI intent")
}

#[cfg(target_arch = "wasm32")]
fn forward_quest_ui_intents(
    applied: Res<AppliedQuestUiSnapshot>,
    mut intents: ResMut<QuestUiIntentQueue>,
    mut pending: ResMut<PendingOperations>,
    mut quest_state: ResMut<QuestUiState>,
) {
    if intents.is_empty() {
        return;
    }
    let batch = intents.drain_intents();
    let active = STATUS.with(|status| {
        let status = status.borrow();
        status.ready && applied.presentation_ready && status.generation == applied.generation
    });
    let sink = INTENT_SINK.with(|sink| sink.borrow().clone());
    for intent in batch {
        let key = intent.pending_key();
        let Some(sink) = sink.as_ref().filter(|_| active) else {
            if let Some(key) = key.as_ref() {
                pending.release(key);
            }
            quest_state.set_feedback("Quest actions are temporarily unavailable", true);
            continue;
        };
        let Ok(json) = encode_quest_intent(&intent, applied.generation) else {
            if let Some(key) = key.as_ref() {
                pending.release(key);
            }
            set_status_error("unsupported shared Quest UI intent");
            quest_state.set_feedback("This Quest action is unavailable in the browser", true);
            continue;
        };
        let response = sink
            .call1(&JsValue::NULL, &JsValue::from_str(&json))
            .ok()
            .and_then(|value| value.as_string())
            .and_then(|value| serde_json::from_str::<WebIntentResult>(&value).ok());
        let Some(response) = response else {
            if let Some(key) = key.as_ref() {
                pending.release(key);
            }
            quest_state.set_feedback("Quest request could not be sent", true);
            continue;
        };
        if !response.accepted {
            if let Some(key) = key.as_ref() {
                pending.release(key);
            }
            quest_state.set_feedback(
                response
                    .error
                    .unwrap_or_else(|| "Quest request was not accepted".to_owned()),
                true,
            );
            continue;
        }
        if let Some(key) = key {
            let Some(request_id) = response
                .request_id
                .filter(|id| !id.is_empty() && id.len() <= 96)
            else {
                pending.release(&key);
                set_status_error("Quest host accepted without a valid request ID");
                quest_state.set_feedback("Quest request could not be tracked", true);
                continue;
            };
            if !pending.bind_external_quest_request_id(key.clone(), &request_id) {
                pending.release(&key);
                set_status_error("Quest host request ID could not be bound");
                quest_state.set_feedback("Quest request could not be tracked", true);
            }
        }
    }
}

fn quest_action_stamp_message(stamp: &mir2_client_bevy::portable_quest_ui::QuestWorldStamp) -> serde_json::Value {
    serde_json::json!({
        "generation": stamp.generation, "revision": stamp.revision,
        "connectionGeneration": stamp.connection_generation,
        "sessionGeneration": stamp.session_generation,
        "sceneRevision": stamp.scene_revision,
        "playerObjectId": stamp.player_object_id,
        "mapFileName": stamp.map_file_name,
    })
}

fn quest_presentation_message(
    action: &mir2_client_bevy::portable_quest_ui::QuestPresentationAction,
    context: &mir2_client_bevy::portable_quest_ui::QuestWorldContext,
    tracker: &QuestTracker,
) -> Option<serde_json::Value> {
    use mir2_client_bevy::portable_quest_ui::QuestPresentationAction as Action;
    if !action.current(context, tracker) { return None; }
    let mut message = quest_action_stamp_message(action.stamp());
    match action {
        Action::OpenDestinationMap(_) => message["type"] = serde_json::json!("openDestinationMap"),
        Action::AttackTarget { object_id, .. } | Action::AttackQuestTarget { object_id, .. } => {
            message["type"] = serde_json::json!(if matches!(action, Action::AttackQuestTarget { .. }) {
                "attackQuestTarget"
            } else { "attackTarget" });
            message["objectId"] = serde_json::json!(object_id);
        }
        Action::PickUpObject { object_id, x, y, .. } => {
            message["type"] = serde_json::json!("pickUpObject");
            message["objectId"] = serde_json::json!(object_id);
            message["x"] = serde_json::json!(x); message["y"] = serde_json::json!(y);
        }
        Action::PickUpTile { x, y, .. } => {
            message["type"] = serde_json::json!("pickUpTile");
            message["x"] = serde_json::json!(x); message["y"] = serde_json::json!(y);
        }
    }
    Some(message)
}

fn quest_route_message(
    intent: mir2_client_bevy::quest_ui::QuestRouteNavigationIntent,
    stamp: &mir2_client_bevy::portable_quest_ui::QuestWorldStamp,
    context: &mir2_client_bevy::portable_quest_ui::QuestWorldContext,
    tracker: &QuestTracker, state: &mir2_client_bevy::quest_ui::QuestUiState,
    journey: Option<&mir2_client_bevy::quest_journey::JourneyView>,
) -> Option<serde_json::Value> {
    use mir2_client_bevy::quest_ui::QuestRouteTarget;
    use mir2_client_bevy::quest_supplies::SupplyVendor;
    if context.stamp.as_ref() != Some(stamp) || context.self_tile().is_none()
        || !mir2_client_bevy::quest_ui::quest_route_intent_is_current(
            intent, tracker, state, journey, context.big_map.as_ref()) { return None; }
    let route_target = match intent.target {
        QuestRouteTarget::Entrance => serde_json::json!({ "type": "entrance" }),
        QuestRouteTarget::HuntRegion { monster_index, radius } => serde_json::json!({
            "type": "huntRegion", "monsterIndex": monster_index, "radius": radius }),
        QuestRouteTarget::Supply { vendor } => serde_json::json!({ "type": "supply", "vendor": match vendor {
            SupplyVendor::Potions => "potions", SupplyVendor::General => "general", SupplyVendor::Poison => "poison",
        } }),
    };
    let mut message = quest_action_stamp_message(stamp);
    message["type"] = serde_json::json!("navigateQuestRoute");
    message["questIndex"] = serde_json::json!(intent.quest_index);
    message["resetEpoch"] = serde_json::json!(intent.reset_epoch);
    message["mapIndex"] = serde_json::json!(intent.map_index);
    message["x"] = serde_json::json!(intent.x); message["y"] = serde_json::json!(intent.y);
    message["routeTarget"] = route_target;
    Some(message)
}

#[cfg(target_arch = "wasm32")]
fn forward_quest_presentation_actions(
    applied: Res<AppliedQuestUiSnapshot>,
    context: Res<mir2_client_bevy::portable_quest_ui::QuestWorldContext>,
    mut actions: ResMut<mir2_client_bevy::portable_quest_ui::QuestPresentationActionQueue>,
    mut quest_state: ResMut<QuestUiState>,
    routing: (ResMut<mir2_client_bevy::quest_ui::QuestRouteNavigationIntentQueue>,
        ResMut<mir2_client_bevy::portable_quest_ui::QuestRouteWorldStamp>,
        Res<QuestTracker>, Res<QuestGuidance>, Res<CompletedQuestTracker>,
        Res<NewcomerJourneyCatalog>, Res<UiReadModel>),
) {
    let (mut routes, mut route_stamp, tracker, guidance, completed, catalog, read_model) = routing;
    let (stamp, message) = if let Some(action) = actions.take() {
        let Some(message) = quest_presentation_message(&action, &context, &tracker) else { return; };
        (action.stamp().clone(), message)
    } else {
        let Some(intent) = routes.take() else { route_stamp.0 = None; return; };
        let Some(stamp) = route_stamp.0.take() else { return; };
        let journey = catalog.derive(&guidance, &tracker, &completed, &read_model.player);
        let Some(message) = quest_route_message(intent, &stamp, &context, &tracker, &quest_state, journey.as_ref()) else { return; };
        (stamp, message)
    };
    let active = STATUS.with(|status| {
        let status = status.borrow();
        status.ready && applied.presentation_ready
            && status.generation == stamp.generation && status.revision == stamp.revision
    });
    if !active || !applied.in_game || !applied.host_visible || !applied.has_snapshot
        || applied.generation != stamp.generation || applied.revision != stamp.revision
        || context.inventory.is_none() || context.player.is_none() || context.stamp.as_ref() != Some(&stamp)
        || PENDING_SNAPSHOT.with(|pending| pending.borrow().is_some()) {
        return;
    }
    let sink = INTENT_SINK.with(|sink| sink.borrow().clone());
    let Some(sink) = sink else { return; };
    let response = sink.call1(&JsValue::NULL, &JsValue::from_str(&message.to_string()))
        .ok().and_then(|value| value.as_string())
        .and_then(|value| serde_json::from_str::<WebIntentResult>(&value).ok());
    if !response.is_some_and(|result| result.accepted) {
        quest_state.set_feedback("The Quest world action is no longer current", true);
    }
}

impl WebQuestUiSnapshot {
    fn is_newer_than(&self, generation: u64, revision: u64) -> bool {
        self.generation > generation || (self.generation == generation && self.revision > revision)
    }
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.generation == 0
            || self.generation > MAX_SAFE_JS_INTEGER
            || self.revision > MAX_SAFE_JS_INTEGER
            || self.open_revision > MAX_SAFE_JS_INTEGER
        {
            return Err("invalid generation or revision");
        }
        if !matches!(
            self.profile.as_str(),
            "crystal" | "newcomer-v1" | "newcomer-v2"
        ) {
            return Err("invalid quest profile");
        }
        if self.in_game && self.player.is_none() {
            return Err("missing player read model");
        }
        if self.dialog.is_open && self.dialog.npc_object_id.is_none() {
            return Err("open dialog lacks NPC identity");
        }
        if self
            .hud_bar_plan
            .as_ref()
            .is_some_and(|commit| !commit.valid())
        {
            return Err("invalid HUD bar draw-plan commit");
        }
        if self.world_context.as_ref().is_some_and(|context| !context.validate(self.generation, self.revision)) {
            return Err("invalid Quest world context");
        }
        let mut ids = HashSet::new();
        if self
            .quests
            .iter()
            .any(|quest| quest.quest_index <= 0 || !ids.insert(quest.quest_index))
        {
            return Err("invalid or duplicate quest index");
        }
        Ok(())
    }

    pub(crate) fn into_models(self) -> QuestUiModels {
        let quests = QuestTracker {
            active_quests: self.quests.into_iter().map(Quest::from).collect(),
        };
        let mut completed = CompletedQuestTracker::default();
        if self.completed_known {
            completed.replace_authoritative(self.completed_quest_ids);
        }
        let dialog = NpcDialogModel {
            is_open: self.dialog.is_open,
            npc_object_id: self.dialog.npc_object_id,
            npc_name: self.dialog.npc_name,
            lines: self
                .dialog
                .lines
                .into_iter()
                .map(|text| NpcDialogLine { text })
                .collect(),
            options: self
                .dialog
                .options
                .into_iter()
                .map(|option| NpcDialogOption {
                    option_id: option.option_id,
                    label: option.label,
                    enabled: option.enabled,
                })
                .collect(),
        };
        let player = self
            .player
            .map_or_else(PlayerStats::default, |player| PlayerStats {
                hp: player.hp,
                max_hp: player.max_hp,
                mp: player.mp.unwrap_or_default(),
                max_mp: player.max_mp.unwrap_or_default(),
                level: player.level,
                class_name: player.class_name,
                name: player.name,
                experience: player.experience.unwrap_or_default(),
                max_experience: player.max_experience.unwrap_or_default(),
                current_weight: player.current_weight.unwrap_or_default(),
                current_weight_known: player.current_weight.is_some(),
                max_weight: player.max_weight.unwrap_or_default(),
                ..Default::default()
            });
        QuestUiModels {
            quests,
            completed,
            dialog,
            read_model: UiReadModel { player },
        }
    }
}

impl From<WebQuest> for Quest {
    fn from(quest: WebQuest) -> Self {
        Self {
            quest_index: quest.quest_index,
            accept_npc_index: quest.accept_npc_index,
            finish_npc_index: quest.finish_npc_index,
            title: quest.title,
            npc_name: quest.npc_name,
            group: quest.group,
            min_level_needed: quest.min_level_needed,
            detail: QuestDetailText {
                description_lines: quest.detail.description_lines,
                task_description_lines: quest.detail.task_description_lines,
                return_description_lines: quest.detail.return_description_lines,
                completion_description_lines: quest.detail.completion_description_lines,
                time_limit: quest.detail.time_limit,
            },
            status: match quest.status {
                WebQuestStatus::NotStarted => QuestStatus::NotStarted,
                WebQuestStatus::InProgress => QuestStatus::InProgress,
                WebQuestStatus::ReadyToTurnIn => QuestStatus::ReadyToTurnIn,
                WebQuestStatus::Completed => QuestStatus::Completed,
                WebQuestStatus::Failed => QuestStatus::Failed,
                WebQuestStatus::Aborted => QuestStatus::Aborted,
                WebQuestStatus::Other => QuestStatus::Unknown("other".to_owned()),
            },
            objectives: quest
                .objectives
                .into_iter()
                .map(|objective| QuestObjective {
                    objective_id: objective.objective_id,
                    text: objective.text,
                    current: objective.current,
                    target: objective.target,
                })
                .collect(),
            rewards: quest.rewards.into_iter().map(QuestReward::from).collect(),
            unknown_text: quest.unknown_text,
        }
    }
}

impl From<WebQuestReward> for QuestReward {
    fn from(reward: WebQuestReward) -> Self {
        match reward {
            WebQuestReward::Gold { amount } => Self::Gold { amount },
            WebQuestReward::Experience { amount } => Self::Experience { amount },
            WebQuestReward::Item {
                item_id,
                name,
                quantity,
                icon,
                selection_index,
                tooltip_source,
            } => Self::Item {
                item_id,
                name,
                quantity,
                icon,
                selection_index,
                tooltip_source,
            },
            WebQuestReward::Unknown { label } => Self::Unknown { label },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebQuestAck {
    generation: u64,
    operation: String,
    request_id: String,
    quest_index: i32,
    #[serde(default)]
    npc_index: Option<u32>,
    #[serde(default)]
    selected_item_index: Option<i32>,
    success: bool,
}

impl WebQuestAck {
    fn into_shared(self) -> Option<mir2_client_bevy::pending_operations::QuestOperationAck> {
        use mir2_client_bevy::pending_operations::QuestOperationAck;
        if self.generation == 0
            || self.generation > MAX_SAFE_JS_INTEGER
            || self.request_id.is_empty()
            || self.quest_index <= 0
        {
            return None;
        }
        match self.operation.as_str() {
            "acceptQuest" if self.selected_item_index.is_none() => {
                Some(QuestOperationAck::AcceptQuest {
                    request_id: self.request_id,
                    npc_index: self.npc_index?,
                    quest_index: self.quest_index,
                    success: self.success,
                })
            }
            "finishQuest" if self.npc_index.is_none() => Some(QuestOperationAck::FinishQuest {
                request_id: self.request_id,
                quest_index: self.quest_index,
                selected_item_index: self.selected_item_index?,
                success: self.success,
            }),
            "abandonQuest" if self.npc_index.is_none() && self.selected_item_index.is_none() => {
                Some(QuestOperationAck::AbandonQuest {
                    request_id: self.request_id,
                    quest_index: self.quest_index,
                    success: self.success,
                })
            }
            _ => None,
        }
    }
}

/// Display-only names; these identities never authorize combat or quest actions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestNameTargets {
    version: u8,
    known: bool,
    stamp: Option<mir2_client_bevy::portable_quest_ui::QuestWorldStamp>,
    object_ids: Vec<u32>,
}
impl QuestNameTargets {
    fn unknown() -> Self { Self { version: 1, known: false, stamp: None, object_ids: vec![] } }
    fn from_context(world: &mir2_client_bevy::portable_quest_ui::QuestWorldContext,
        host: &QuestUiHostContext, tracker: &QuestTracker, applied_current: bool) -> Self {
        if !applied_current || !world.current(host) { return Self::unknown(); }
        let object_ids = world.raw_entities.iter()
            .filter(|row| row.kind == mir2_client_bevy::entities::EntityKind::Monster && row.dead == Some(false)
                && mir2_client_bevy::crystal_ui::quest_targets::tracker_targets_monster(tracker, &row.name))
            .map(|row| row.object_id).collect();
        Self { version: 1, known: true, stamp: world.stamp.clone(), object_ids }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebQuestUiStatus {
    quest_locale_version: u8,
    language: QuestPresentationLocale,
    ready: bool,
    error: Option<String>,
    captures_pointer: bool,
    quest_log_open: bool,
    frame: u64,
    generation: u64,
    revision: u64,
    open_revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    hp_orb: Option<WebHpOrbStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mp_orb: Option<WebMpOrbStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    experience_bar: Option<WebExperienceBarStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weight_bar: Option<WebWeightBarStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hud_bar_draw_plan: Option<HudBarDrawPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compact: Option<WebCompactQuestStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebHpOrbRect {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

#[cfg(target_arch = "wasm32")]
impl From<HpOrbRect> for WebHpOrbRect {
    fn from(rect: HpOrbRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl From<ExperienceBarRect> for WebExperienceBarRect {
    fn from(rect: ExperienceBarRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl From<WeightBarRect> for WebExperienceBarRect {
    fn from(rect: WeightBarRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebWeightBarStatus {
    supported: bool,
    ready: bool,
    generation: u64,
    revision: u64,
    current_weight: Option<u16>,
    max_weight: Option<u16>,
    slot: Option<WebExperienceBarRect>,
    image: Option<&'static str>,
    source: Option<WebExperienceBarRect>,
    destination: Option<WebExperienceBarRect>,
    layout: Option<WebExperienceBarRect>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebExperienceBarStatus {
    supported: bool,
    ready: bool,
    generation: u64,
    revision: u64,
    experience: Option<i64>,
    max_experience: Option<i64>,
    slot: Option<WebExperienceBarRect>,
    image: Option<&'static str>,
    source: Option<WebExperienceBarRect>,
    destination: Option<WebExperienceBarRect>,
    layout: Option<WebExperienceBarRect>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebHpOrbStatus {
    supported: bool,
    ready: bool,
    generation: u64,
    revision: u64,
    hp: Option<i32>,
    max_hp: Option<i32>,
    hp_only: Option<bool>,
    slot: Option<WebHpOrbSlot>,
    image: Option<&'static str>,
    source: Option<WebHpOrbRect>,
    destination: Option<WebHpOrbRect>,
    layout: Option<WebHpOrbRect>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebMpOrbStatus {
    supported: bool,
    ready: bool,
    generation: u64,
    revision: u64,
    mp: Option<i32>,
    max_mp: Option<i32>,
    hp_only: Option<bool>,
    slot: Option<WebHpOrbSlot>,
    image: Option<&'static str>,
    source: Option<WebHpOrbRect>,
    destination: Option<WebHpOrbRect>,
    layout: Option<WebHpOrbRect>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebCompactQuestActionRect {
    label: &'static str,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebCompactQuestPanelRect {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebCompactQuestStatus {
    current: bool,
    generation: Option<u64>,
    revision: Option<u64>,
    sheet: Option<&'static str>,
    page: Option<&'static str>,
    quest_index: Option<i32>,
    reward_page: Option<usize>,
    text_top: Option<usize>,
    diary_page: Option<usize>,
    action_count: usize,
    text_count: usize,
    min_css_hit_width: Option<f32>,
    min_css_hit_height: Option<f32>,
    min_css_text_size: Option<f32>,
    panel_css_bounds: Option<WebCompactQuestPanelRect>,
    first_failure: Option<&'static str>,
    action_bounds: Vec<WebCompactQuestActionRect>,
}

impl Default for WebQuestUiStatus {
    fn default() -> Self {
        Self {
            quest_locale_version: 1,
            language: QuestPresentationLocale::default(),
            ready: false,
            error: None,
            captures_pointer: false,
            quest_log_open: false,
            frame: 0,
            generation: 0,
            revision: 0,
            open_revision: 0,
            hp_orb: None,
            mp_orb: None,
            experience_bar: None,
            weight_bar: None,
            hud_bar_draw_plan: None,
            compact: None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static PENDING_SNAPSHOT: RefCell<Option<WebQuestUiSnapshot>> = const { RefCell::new(None) };
    static PENDING_ACKS: RefCell<Vec<WebQuestAck>> = const { RefCell::new(Vec::new()) };
    static INTENT_SINK: RefCell<Option<Function>> = const { RefCell::new(None) };
    static STATUS: RefCell<WebQuestUiStatus> = RefCell::new(WebQuestUiStatus::default());
    static NAME_TARGETS: RefCell<QuestNameTargets> = RefCell::new(QuestNameTargets::unknown());
    static WORLD_CONTROL_RECTS: RefCell<mir2_client_bevy::portable_quest_ui::QuestWorldControlRects> =
        RefCell::new(mir2_client_bevy::portable_quest_ui::QuestWorldControlRects::default());
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = setMir2QuestUiSnapshot)]
pub fn set_mir2_quest_ui_snapshot(json: String) -> bool {
    let Ok(snapshot) = serde_json::from_str::<WebQuestUiSnapshot>(&json) else {
        set_status_error("invalid quest UI snapshot");
        return false;
    };
    if let Err(error) = snapshot.validate() {
        set_status_error(error);
        return false;
    }
    let stale = STATUS.with(|status| {
        let status = status.borrow();
        !snapshot.is_newer_than(status.generation, status.revision)
    }) || PENDING_SNAPSHOT.with(|pending| {
        pending.borrow().as_ref().is_some_and(|current| {
            !snapshot.is_newer_than(current.generation, current.revision)
        })
    });
    if stale {
        return false;
    }
    PENDING_SNAPSHOT.with(|pending| *pending.borrow_mut() = Some(snapshot));
    true
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = getMir2QuestWorldContextVersion)]
pub fn get_mir2_quest_world_context_version() -> u32 { 2 }

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = getMir2QuestWorldControlRects)]
pub fn get_mir2_quest_world_control_rects() -> String {
    let pending = PENDING_SNAPSHOT.with(|pending| pending.borrow().is_some());
    WORLD_CONTROL_RECTS.with(|rects| {
        let rects = rects.borrow();
        let current = !pending && INTENT_SINK.with(|sink| sink.borrow().is_some())
            && rects.stamp.as_ref().is_some_and(|stamp| STATUS.with(|status| {
            let status = status.borrow();
            status.ready && status.generation == stamp.generation && status.revision == stamp.revision
        }));
        // Do not relabel previous-frame rectangles with a pending snapshot.
        if current { serde_json::to_string(&*rects) } else {
            serde_json::to_string(&mir2_client_bevy::portable_quest_ui::QuestWorldControlRects::default())
        }.unwrap_or_else(|_| "null".to_owned())
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn quest_name_targets_json() -> String {
    let pending = PENDING_SNAPSHOT.with(|pending| pending.borrow().is_some());
    NAME_TARGETS.with(|published| {
        let targets = published.borrow();
        let current = !pending && targets.known && targets.stamp.as_ref().is_some_and(|stamp| STATUS.with(|status| {
            let status = status.borrow(); status.generation == stamp.generation && status.revision == stamp.revision
        }));
        let json = if current { serde_json::to_string(&*targets) } else { serde_json::to_string(&QuestNameTargets::unknown()) };
        json.unwrap_or_else(|_| "{\"version\":1,\"known\":false,\"stamp\":null,\"objectIds\":[]}".into())
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = setMir2QuestUiOperationAck)]
pub fn set_mir2_quest_ui_operation_ack(json: String) -> bool {
    let Ok(ack) = serde_json::from_str::<WebQuestAck>(&json) else {
        return false;
    };
    if ack.clone().into_shared().is_none() {
        return false;
    }
    PENDING_ACKS.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.len() >= 128 {
            return false;
        }
        pending.push(ack);
        true
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = setMir2QuestUiIntentSink)]
pub fn set_mir2_quest_ui_intent_sink(sink: Function) {
    INTENT_SINK.with(|current| *current.borrow_mut() = Some(sink));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = clearMir2QuestUiIntentSink)]
pub fn clear_mir2_quest_ui_intent_sink() {
    INTENT_SINK.with(|current| *current.borrow_mut() = None);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = getMir2QuestUiStatus)]
pub fn get_mir2_quest_ui_status() -> String {
    STATUS.with(|status| serde_json::to_string(&*status.borrow()).unwrap_or_default())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = getMir2HudBarDrawPlanVersion)]
pub fn get_mir2_hud_bar_draw_plan_version() -> u32 {
    1
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = getMir2HudBarDrawPlan)]
pub fn get_mir2_hud_bar_draw_plan() -> String {
    STATUS.with(|status| {
        serde_json::to_string(&status.borrow().hud_bar_draw_plan)
            .unwrap_or_else(|_| "null".to_owned())
    })
}

#[cfg(target_arch = "wasm32")]
fn set_status_error(message: &str) {
    STATUS.with(|status| {
        let mut status = status.borrow_mut();
        status.ready = false;
        status.captures_pointer = false;
        status.error = Some(message.to_owned());
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quest_world_context_rejects_stale_or_partial_authority_and_preserves_zero_gold() {
        let base = json!({"generation":7,"revision":9,"connectionGeneration":2,
            "sessionGeneration":3,"sceneRevision":4,"playerObjectId":1000,"mapFileName":"0",
            "mapIndex":null,"entities":[],"selectedObjectId":null,"groundDrops":[],
            "inventory":{"capacity":46,"gold":0,"items":[]},
            "player":{"level":5,"className":"Taoist","gender":"female","gold":0,
                "currentWeight":0,"currentWeightKnown":false,"maxWeight":100},
            "knownSkills":[]});
        let parse = |value| serde_json::from_value::<WebQuestWorldContext>(value).unwrap();
        let valid = parse(base.clone());
        assert!(valid.validate(7, 9));
        assert!(!valid.validate(7, 10));
        let projected = valid.into_resource();
        assert_eq!(projected.inventory.unwrap().gold, 0);
        assert_eq!(projected.player.unwrap().gender.as_deref(), Some("female"));
        assert!(projected.skills.is_some());
        let mut changed = base.clone();
        changed["player"]["gold"] = json!(1);
        assert!(!parse(changed).validate(7, 9));
        let mut changed = base.clone();
        changed["mapFileName"] = json!("0.map");
        assert!(!parse(changed).validate(7, 9));
        let mut changed = base.clone();
        changed["inventory"]["capacity"] = json!(47);
        assert!(!parse(changed).validate(7, 9));
        let mut changed = base.clone();
        changed["player"]["gender"] = json!(null);
        assert!(serde_json::from_value::<WebQuestWorldContext>(changed).is_err());
        for field in ["capacity", "gold", "items"] {
            let mut changed = base.clone();
            changed["inventory"].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<WebQuestWorldContext>(changed).is_err(),
                "missing {field} must not become a complete Quest inventory");
        }
    }

    fn quest_world_v2_json() -> serde_json::Value {
        json!({"generation":7,"revision":9,"connectionGeneration":2,"sessionGeneration":3,
            "sceneRevision":4,"playerObjectId":1000,"mapFileName":"0","mapIndex":null,
            "entities":[{"objectId":1000,"kind":"selfPlayer","name":"Player","x":100,"y":200,
                "direction":null,"level":5,"dead":null,"hp":null,"maxHp":null},
                {"objectId":44,"kind":"monster","name":"Hen","x":102,"y":200,
                "direction":null,"level":null,"dead":false,"hp":12,"maxHp":20}],
            "selectedObjectId":44,"groundDrops":[{"objectId":80,"name":"Gold","x":102,"y":201,
                "quantity":2,"sourceMonster":"Hen"}],"inventory":{"capacity":46,"gold":0,"items":[]},
            "player":{"level":5,"className":"Taoist","gender":"female","gold":0,
                "currentWeight":0,"currentWeightKnown":false,"maxWeight":100},"knownSkills":[]})
    }

    #[test]
    fn quest_world_v2_requires_complete_nullable_rows_and_unique_bounded_object_ids() {
        let base = quest_world_v2_json();
        let parse = |value| serde_json::from_value::<WebQuestWorldContext>(value).unwrap();
        assert!(parse(base.clone()).validate(7, 9));
        for field in ["mapIndex", "entities", "selectedObjectId", "groundDrops"] {
            let mut changed = base.clone(); changed.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<WebQuestWorldContext>(changed).is_err(), "missing {field}");
        }
        for field in ["direction", "level", "dead", "hp", "maxHp"] {
            let mut changed = base.clone(); changed["entities"][1].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<WebQuestWorldContext>(changed).is_err(), "missing {field}");
        }
        let mut changed = base.clone();
        changed["entities"][1]["dead"] = json!(null);
        let projected = parse(changed).into_resource();
        assert_eq!(projected.raw_entities[1].dead, None);
        assert!(projected.target.target.is_none());
        assert_eq!(projected.big_map.as_ref().unwrap().current_map_index, None);
        for (field, invalid) in [("objectId", json!(0)), ("objectId", json!(1000))] {
            let mut changed = base.clone(); changed["entities"][1][field] = invalid;
            assert!(!parse(changed).validate(7, 9));
        }
        for (field, invalid) in [("objectId", json!(44)), ("objectId", json!(0)), ("quantity", json!(0))] {
            let mut changed = base.clone(); changed["groundDrops"][0][field] = invalid;
            assert!(!parse(changed).validate(7, 9));
        }
        let mut changed = base.clone();
        let row = changed["entities"][1].clone();
        changed["entities"] = json!(vec![row; 513]);
        assert!(!parse(changed).validate(7, 9));
        let mut changed = base.clone();
        let row = changed["groundDrops"][0].clone();
        changed["groundDrops"] = json!(vec![row; 513]);
        assert!(!parse(changed).validate(7, 9));
    }

    fn quest_world_hunt_tracker() -> QuestTracker {
        QuestTracker { active_quests: vec![Quest {
            quest_index: 23, accept_npc_index: None, finish_npc_index: None,
            title: "Hunt".into(), npc_name: None, group: None, min_level_needed: 0,
            detail: Default::default(), status: QuestStatus::InProgress,
            objectives: vec![QuestObjective { objective_id: "hen".into(), text: "Defeat Hen".into(), current: 0, target: 2 }],
            rewards: vec![], unknown_text: None,
        }] }
    }

    #[test]
    fn quest_world_local_messages_preserve_origin_and_recheck_life_tile_and_revision() {
        use mir2_client_bevy::portable_quest_ui::{QuestPresentationAction as Action, QuestPresentationActionQueue};
        let mut context = serde_json::from_value::<WebQuestWorldContext>(quest_world_v2_json()).unwrap().into_resource();
        let tracker = quest_world_hunt_tracker();
        let stamp = context.stamp.clone().unwrap();
        let actions = [Action::OpenDestinationMap(stamp.clone()),
            Action::AttackTarget { stamp: stamp.clone(), object_id: 44 },
            Action::AttackQuestTarget { stamp: stamp.clone(), object_id: 44 },
            Action::PickUpObject { stamp: stamp.clone(), object_id: 80, x: 102, y: 201 },
            Action::PickUpTile { stamp: stamp.clone(), x: 100, y: 200 }];
        let names = ["openDestinationMap", "attackTarget", "attackQuestTarget", "pickUpObject", "pickUpTile"];
        let mut queue = QuestPresentationActionQueue::default();
        for (action, expected_type) in actions.iter().zip(names) {
            assert!(queue.push(action.clone()));
            assert!(!queue.push(action.clone()));
            let consumed = queue.take().unwrap();
            let message = quest_presentation_message(&consumed, &context, &tracker).unwrap();
            assert_eq!(message["type"], expected_type);
            assert_eq!(message["revision"], 9);
            assert_eq!(message["sceneRevision"], 4);
            if expected_type == "pickUpObject" { assert_eq!(message["x"], 102); assert_eq!(message["y"], 201); }
            if expected_type == "pickUpTile" { assert_eq!(message["x"], 100); assert_eq!(message["y"], 200); }
            assert!(queue.take().is_none());
        }
        context.raw_entities[1].dead = None;
        assert!(quest_presentation_message(&actions[2], &context, &tracker).is_none());
        context.raw_entities[1].dead = Some(false); context.raw_entities[1].hp = None;
        assert!(quest_presentation_message(&actions[2], &context, &tracker).is_none());
        context.raw_entities[1].max_hp = None;
        assert!(quest_presentation_message(&actions[2], &context, &tracker).is_some());
        context.rebuild_projections();
        assert!(quest_presentation_message(&actions[1], &context, &tracker).is_none());
        context.raw_entities[0].x += 1;
        assert!(quest_presentation_message(&actions[4], &context, &tracker).is_none());
        context.ground_drops[0].x += 1;
        assert!(quest_presentation_message(&actions[3], &context, &tracker).is_none());
        context.stamp.as_mut().unwrap().revision += 1;
        assert!(quest_presentation_message(&actions[0], &context, &tracker).is_none());
    }

    #[test]
    fn quest_world_route_output_revalidates_shared_supply_and_unknown_map() {
        use mir2_client_bevy::quest_supplies::SupplyVendor;
        use mir2_client_bevy::quest_ui::{QuestRouteNavigationIntent, QuestRouteTarget, QuestUiState};
        let mut context = serde_json::from_value::<WebQuestWorldContext>(quest_world_v2_json()).unwrap().into_resource();
        let destination = SupplyVendor::Potions.destination().unwrap();
        let intent = QuestRouteNavigationIntent { quest_index: 0, reset_epoch: 4,
            map_index: destination.map_index, x: destination.x, y: destination.y,
            target: QuestRouteTarget::Supply { vendor: SupplyVendor::Potions } };
        let stamp = context.stamp.clone().unwrap();
        let mut state = QuestUiState::default();
        state.supply_open = true; state.supply_vendor = Some(SupplyVendor::Potions);
        let tracker = QuestTracker::default();
        assert!(quest_route_message(intent, &stamp, &context, &tracker, &state, None).is_none());
        context.map_index = Some(destination.map_index); context.rebuild_projections();
        let message = quest_route_message(intent, &stamp, &context, &tracker, &state, None).unwrap();
        assert_eq!(message["type"], "navigateQuestRoute");
        assert_eq!(message["routeTarget"], json!({"type":"supply","vendor":"potions"}));
        assert_eq!(message["mapIndex"], destination.map_index);
        assert_eq!(message["resetEpoch"], 4);
        assert!(quest_route_message(QuestRouteNavigationIntent { reset_epoch: 5, ..intent }, &stamp, &context, &tracker, &state, None).is_none());
        let closed = QuestUiState::default();
        assert!(quest_route_message(intent, &stamp, &context, &tracker, &closed, None).is_none());
        context.stamp.as_mut().unwrap().scene_revision += 1;
        assert!(quest_route_message(intent, &stamp, &context, &tracker, &state, None).is_none());
        context.stamp = Some(stamp.clone()); context.map_index = Some(39); context.rebuild_projections();
        let mut hunt = quest_world_hunt_tracker();
        hunt.active_quests[0].quest_index = 2_110_010;
        hunt.active_quests[0].objectives[0].text = "Defeat 4 Skeleton.".into();
        hunt.active_quests[0].objectives[0].target = 4;
        let region = mir2_client_bevy::quest_hunt_regions::active_hunt_regions(&hunt, 39,
            mir2_client_bevy::big_map::BigMapPoint { x: 211, y: 320 }).remove(0);
        let hunt_intent = QuestRouteNavigationIntent { quest_index: 2_110_010, reset_epoch: 4,
            map_index: 39, x: region.center.x, y: region.center.y,
            target: QuestRouteTarget::HuntRegion { monster_index: region.monster_index, radius: region.radius } };
        let message = quest_route_message(hunt_intent, &stamp, &context, &hunt, &closed, None).unwrap();
        assert_eq!(message["routeTarget"], json!({ "type":"huntRegion", "monsterIndex":region.monster_index, "radius":region.radius }));
        assert!(quest_route_message(QuestRouteNavigationIntent { target: QuestRouteTarget::HuntRegion {
            monster_index: region.monster_index, radius: region.radius + 1 }, ..hunt_intent },
            &stamp, &context, &hunt, &closed, None).is_none());
    }

    #[test]
    fn m8_strict_abi1_rejects_extended_player_and_cleanup_cannot_erase_full_hud() {
        let mut old=json!({"generation":7,"revision":999,"inGame":false,"hostVisible":false,
            "questLogOpen":false,"openRevision":1,"blocksGameplayKeys":false,"turnInBlocked":false,
            "profile":"crystal","quests":[],"completedKnown":false,"completedQuestIds":[],
            "dialog":{"isOpen":false,"hasInput":false},"player":{"hp":0,"maxHp":0,"level":0}});
        old["player"]["crystalStats"]=json!([{"stat":0,"value":9}]);
        let rejected=serde_json::from_value::<WebQuestUiSnapshot>(old.clone()).unwrap_err();
        assert!(rejected.to_string().contains("unknown field `crystalStats`"));
        old["player"].as_object_mut().unwrap().remove("crystalStats");
        let cleanup=serde_json::from_value::<WebQuestUiSnapshot>(old).unwrap().into_models();
        assert_eq!(cleanup.read_model.player.gold,0); // Actual ABI1/default loss boundary.
        let mut model=UiReadModel {player:PlayerStats{gold:32,name:Some("Full".into()),
            crystal_stats:Some(vec![mir2_client_bevy::read_model::CrystalPlayerStatModel{stat:0,value:9}]),..Default::default()}};
        let expected=model.player.clone();let mut ingress=mir2_client_bevy::read_model::UiReadModelIngress::default();
        assert!(ingress.apply_full(&mut model,7,1,Some(expected.clone())));
        assert!(!ingress.apply_legacy_quest(&mut model,7,999,cleanup.read_model.player));
        assert_eq!(model.player,expected);
    }

    #[test]
    fn draw_plan_route_resolves_four_required_host_resources_without_painters() {
        let mut app = App::new();
        install_bar_route(&mut app, true);
        assert!(app.world().resource::<HudBarDrawPlanRoute>().0);
        app.add_systems(
            Update,
            |_exp: Res<ExperienceBarHostContext>,
             _weight: Res<WeightBarHostContext>,
             _exp_observed: Res<ExperienceBarObservation>,
             _weight_observed: Res<WeightBarObservation>| {},
        );
        app.update();
        assert!(!app.world().resource::<ExperienceBarObservation>().ready);
        assert!(!app.world().resource::<WeightBarObservation>().ready);
    }

    #[test]
    fn draw_plan_commit_is_exact_optional_and_bounded() {
        let mut input = json!({
            "generation": 5, "revision": 9, "inGame": true, "hostVisible": true,
            "questLogOpen": false, "openRevision": 0, "blocksGameplayKeys": false,
            "turnInBlocked": false, "profile": "crystal", "quests": [],
            "completedKnown": false, "completedQuestIds": [],
            "dialog": { "isOpen": false, "lines": [], "options": [], "hasInput": false },
            "player": { "hp": 0, "maxHp": 100, "level": 1 },
        });
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(input.clone())
            .unwrap()
            .hud_bar_plan
            .is_none());
        input["hudBarPlan"] = json!({"lifetime":"r1-p1", "experienceToken":"e1",
            "experienceSequence":1, "weightToken":"w1", "weightSequence":1});
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(input.clone())
            .unwrap()
            .validate()
            .is_ok());
        input["hudBarPlan"]["weightToken"] = json!("é");
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(input.clone())
            .unwrap()
            .validate()
            .is_err());
        input["hudBarPlan"]["weightToken"] = json!("w1");
        input["hudBarPlan"]["unexpected"] = json!(1);
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(input).is_err());
    }

    #[test]
    fn optional_experience_pair_preserves_unknown_and_real_max_one() {
        let mut input = json!({
            "generation": 5, "revision": 9, "inGame": true, "hostVisible": false,
            "questLogOpen": false, "openRevision": 0, "blocksGameplayKeys": false,
            "turnInBlocked": false, "profile": "crystal", "quests": [],
            "completedKnown": false, "completedQuestIds": [],
            "dialog": { "isOpen": false, "lines": [], "options": [], "hasInput": false },
            "player": { "hp": 0, "maxHp": 100, "level": 1 },
        });
        let old: WebQuestUiSnapshot = serde_json::from_value(input.clone()).unwrap();
        assert!(old.experience_bar_slot.is_none());
        assert_eq!(old.into_models().read_model.player.max_experience, 0);
        input["player"]["experience"] = json!(0);
        input["player"]["maxExperience"] = json!(1);
        input["experienceBarSlot"] = json!({ "left": 9, "top": 759, "width": 1004, "height": 8 });
        let current: WebQuestUiSnapshot = serde_json::from_value(input).unwrap();
        assert_eq!(
            current.experience_bar_slot.map(|slot| slot.width),
            Some(1004.0)
        );
        assert_eq!(current.validate(), Ok(()));
        let model = current.into_models().read_model;
        assert_eq!(
            (model.player.experience, model.player.max_experience),
            (0, 1)
        );
        let status = serde_json::to_value(WebQuestUiStatus {
            experience_bar: Some(WebExperienceBarStatus {
                supported: true,
                ready: false,
                generation: 5,
                revision: 9,
                experience: None,
                max_experience: None,
                slot: None,
                image: None,
                source: None,
                destination: None,
                layout: None,
            }),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(status["experienceBar"]["supported"], true);
        assert_eq!(status["experienceBar"]["ready"], false);
        assert!(status["experienceBar"]["slot"].is_null());
    }

    #[test]
    fn optional_weight_pair_preserves_real_empty_bag_and_rejects_bad_u16() {
        let mut input = json!({
            "generation": 5, "revision": 9, "inGame": true, "hostVisible": false,
            "questLogOpen": false, "openRevision": 0, "blocksGameplayKeys": false,
            "turnInBlocked": false, "profile": "crystal", "quests": [],
            "completedKnown": false, "completedQuestIds": [],
            "dialog": { "isOpen": false, "lines": [], "options": [], "hasInput": false },
            "player": { "hp": 0, "maxHp": 100, "level": 1 },
        });
        let old: WebQuestUiSnapshot = serde_json::from_value(input.clone()).unwrap();
        assert!(old.weight_bar_slot.is_none());
        assert_eq!(
            old.player.as_ref().and_then(|player| player.current_weight),
            None
        );
        input["player"]["currentWeight"] = json!(0);
        input["player"]["maxWeight"] = json!(1);
        input["weightBarSlot"] = json!({ "left": 919, "top": 721, "width": 76, "height": 12 });
        let current: WebQuestUiSnapshot = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(
            current
                .player
                .as_ref()
                .and_then(|player| player.current_weight),
            Some(0)
        );
        assert_eq!(
            current.player.as_ref().and_then(|player| player.max_weight),
            Some(1)
        );
        assert_eq!(current.weight_bar_slot.map(|slot| slot.top), Some(721.0));
        let model = current.into_models().read_model;
        assert_eq!(
            (model.player.current_weight, model.player.max_weight),
            (0, 1)
        );
        input["player"]["currentWeight"] = json!(65536);
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(input).is_err());
        let status = serde_json::to_value(WebQuestUiStatus {
            weight_bar: Some(WebWeightBarStatus {
                supported: true,
                ready: false,
                generation: 5,
                revision: 9,
                current_weight: None,
                max_weight: None,
                slot: None,
                image: None,
                source: None,
                destination: None,
                layout: None,
            }),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(status["weightBar"]["supported"], true);
        assert_eq!(status["weightBar"]["ready"], false);
    }

    #[test]
    fn optional_hp_slot_and_status_do_not_change_quest_ready_or_capture_contract() {
        let snapshot: WebQuestUiSnapshot = serde_json::from_value(json!({
            "generation": 5, "revision": 9, "inGame": true, "hostVisible": false,
            "questLogOpen": false, "openRevision": 0, "blocksGameplayKeys": false,
            "turnInBlocked": false, "profile": "crystal", "quests": [],
            "completedKnown": false, "completedQuestIds": [],
            "dialog": { "isOpen": false, "lines": [], "options": [], "hasInput": false },
            "player": { "hp": 0, "maxHp": 100, "level": 25, "className": "Warrior" },
            "hpOrbSlot": { "left": 12.0, "top": 646.0 }
        }))
        .expect("optional slot accepted on existing snapshot");
        assert_eq!(snapshot.hp_orb_slot.map(|slot| slot.left), Some(12.0));
        assert_eq!(snapshot.validate(), Ok(()));
        let models = snapshot.into_models();
        assert_eq!(models.read_model.player.hp, 0);
        assert!(mir2_client_bevy::crystal_ui::hud_orb::crystal_hp_only(
            &models.read_model
        ));

        let unavailable = serde_json::to_value(WebQuestUiStatus::default()).unwrap();
        assert!(
            unavailable.get("hpOrb").is_none(),
            "lean/old status omits the capability"
        );
        assert!(unavailable.get("mpOrb").is_none());
        let supported = serde_json::to_value(WebQuestUiStatus {
            hp_orb: Some(WebHpOrbStatus {
                supported: true,
                ready: false,
                generation: 0,
                revision: 0,
                hp: None,
                max_hp: None,
                hp_only: None,
                slot: None,
                image: None,
                source: None,
                destination: None,
                layout: None,
            }),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(supported["hpOrb"]["supported"], true);
        assert_eq!(supported["hpOrb"]["ready"], false);
        assert!(supported["hpOrb"]["slot"].is_null());
        assert_eq!(supported["capturesPointer"], false);
    }

    #[test]
    fn optional_mp_preserves_missing_vs_zero_and_unready_status_is_null() {
        let make = |player: serde_json::Value| -> WebQuestUiSnapshot {
            serde_json::from_value(json!({
                "generation": 7, "revision": 8, "inGame": true,
                "hostVisible": false, "questLogOpen": false, "openRevision": 0,
                "blocksGameplayKeys": false, "turnInBlocked": false,
                "profile": "crystal", "quests": [], "completedKnown": false,
                "completedQuestIds": [], "dialog": {"isOpen": false, "lines": [],
                    "options": [], "hasInput": false}, "player": player,
                "hpOrbSlot": {"left": 0.0, "top": 646.0}
            }))
            .unwrap()
        };
        let old = make(json!({"hp": 30, "maxHp": 40, "level": 30, "className": "Wizard"}));
        assert_eq!(old.player.as_ref().unwrap().mp, None);
        assert_eq!(old.into_models().read_model.player.mp, 0);
        let known_zero = make(json!({"hp": 30, "maxHp": 40, "mp": 0, "maxMp": 70,
            "level": 30, "className": "Wizard"}));
        assert_eq!(known_zero.validate(), Ok(()));
        assert_eq!(known_zero.player.as_ref().unwrap().mp, Some(0));
        assert_eq!(known_zero.player.as_ref().unwrap().max_mp, Some(70));
        assert_eq!(known_zero.into_models().read_model.player.max_mp, 70);
        let partial = make(json!({"hp": 30, "maxHp": 40, "mp": 20,
            "level": 30, "className": "Wizard"}));
        assert_eq!(partial.player.as_ref().unwrap().mp, Some(20));
        assert_eq!(partial.player.as_ref().unwrap().max_mp, None);
        let supported = serde_json::to_value(WebQuestUiStatus {
            mp_orb: Some(WebMpOrbStatus {
                supported: true,
                ready: false,
                generation: 0,
                revision: 0,
                mp: None,
                max_mp: None,
                hp_only: None,
                slot: None,
                image: None,
                source: None,
                destination: None,
                layout: None,
            }),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(supported["mpOrb"]["supported"], true);
        for field in [
            "mp",
            "maxMp",
            "hpOnly",
            "slot",
            "image",
            "source",
            "destination",
            "layout",
        ] {
            assert!(
                supported["mpOrb"][field].is_null(),
                "{field} is null until measured ready"
            );
        }
        assert_eq!(supported["capturesPointer"], false);
    }

    #[test]
    fn shared_ui_uses_only_primary_window_and_active_transparent_layer_camera() {
        let mut app = App::new();
        let primary = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let ui_window = ui_target_window(&mut app, true);
        assert_eq!(ui_window, primary);
        assert!(app.world().get::<QuestUiHostWindow>(primary).is_some());
        let world = app.world_mut();
        assert_eq!(world.query::<&Window>().iter(world).count(), 1);

        let ui_camera = ui_target_camera(&mut app, ui_window, true);
        let camera = app.world().get::<Camera>(ui_camera).unwrap();
        assert!(camera.is_active);
        assert_eq!(camera.order, 1);
        assert!(
            matches!(camera.clear_color, ClearColorConfig::Custom(color) if color == Color::NONE)
        );
        assert_eq!(
            app.world().get::<RenderLayers>(ui_camera),
            Some(&RenderLayers::layer(30))
        );
        assert!(matches!(app.world().get::<RenderTarget>(ui_camera),
            Some(RenderTarget::Window(WindowRef::Entity(target))) if *target == primary));
    }

    #[test]
    fn ordinary_ui_keeps_sibling_window_and_primary_unmarked() {
        let mut app = App::new();
        let primary = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let ui_window = ui_target_window(&mut app, false);
        assert_ne!(ui_window, primary);
        assert!(app.world().get::<QuestUiHostWindow>(primary).is_none());
        let world = app.world_mut();
        assert_eq!(world.query::<&Window>().iter(world).count(), 2);
        assert_eq!(
            app.world()
                .get::<Window>(ui_window)
                .unwrap()
                .canvas
                .as_deref(),
            Some("#mir2-quest-ui-canvas")
        );
        let ui_camera = ui_target_camera(&mut app, ui_window, false);
        assert_eq!(app.world().get::<Camera>(ui_camera).unwrap().order, 0);
    }

    #[test]
    fn quest_locale_dto_distinguishes_missing_from_present_invalid_and_publishes_initial_capability() {
        let base = json!({ "generation": 4, "revision": 10, "inGame": true,
            "hostVisible": true, "questLogOpen": true, "openRevision": 2,
            "blocksGameplayKeys": false, "turnInBlocked": false, "profile": "crystal",
            "quests": [], "completedKnown": true, "completedQuestIds": [],
            "dialog": { "isOpen": false, "lines": [], "options": [], "hasInput": false }, "player": { "hp": 10, "maxHp": 10, "level": 1 } });
        let missing: WebQuestUiSnapshot = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(missing.language, QuestPresentationLocale::default());
        for language in ["en", "zh-CN", "es", "pt-BR"] {
            let mut value = base.clone(); value["language"] = json!(language);
            let snapshot: WebQuestUiSnapshot = serde_json::from_value(value).unwrap();
            assert_eq!(snapshot.language.0.code(), language);
            assert_eq!(snapshot.validate(), Ok(()), "valid locale DTO {language}");
            assert!(snapshot.is_newer_than(4, 9));
            assert!(!snapshot.is_newer_than(4, 10));
            assert!(!snapshot.is_newer_than(5, 0));
            assert!(snapshot.is_newer_than(3, 100));
        }
        for invalid in [json!(null), json!(4), json!(true), json!({}), json!([]),
            json!("en-US"), json!("EN"), json!("zh"), json!("pt"), json!("es-ES"), json!("")] {
            let mut value = base.clone(); value["language"] = invalid.clone();
            assert!(serde_json::from_value::<WebQuestUiSnapshot>(value).is_err(), "{invalid}");
        }
        let original = serde_json::to_string(&base).unwrap();
        let duplicate = format!("{{\"language\":\"en\",\"language\":\"es\",{}", &original[1..]);
        assert!(serde_json::from_str::<WebQuestUiSnapshot>(&duplicate).is_err());
        let mut unknown = base; unknown["locale"] = json!("en");
        assert!(serde_json::from_value::<WebQuestUiSnapshot>(unknown).is_err());
        let initial = serde_json::to_value(WebQuestUiStatus::default()).unwrap();
        assert_eq!(initial["questLocaleVersion"], 1);
        assert_eq!(initial["language"], "zh-CN");
        assert_eq!(initial["ready"], false);
    }

    fn snapshot_with(quest: serde_json::Value) -> WebQuestUiSnapshot {
        serde_json::from_value(json!({
            "generation": 1,
            "revision": 2,
            "inGame": true,
            "hostVisible": true,
            "questLogOpen": true,
            "openRevision": 1,
            "blocksGameplayKeys": false,
            "turnInBlocked": false,
            "profile": "newcomer-v2",
            "quests": [quest],
            "completedKnown": true,
            "completedQuestIds": [],
            "dialog": {"isOpen": false, "lines": [], "options": [], "hasInput": false},
            "player": {"hp": 25, "maxHp": 30, "level": 4, "className": "Warrior"}
        }))
        .expect("valid snapshot")
    }

    #[test]
    fn typed_reward_and_zero_endpoint_survive_projection() {
        let snapshot = snapshot_with(json!({
            "questIndex": 7,
            "acceptNpcIndex": 0,
            "finishNpcIndex": null,
            "title": "A Quest",
            "status": "readyToTurnIn",
            "rewards": [{"type": "item", "itemId": "iron-sword", "name": "Iron Sword",
                         "quantity": 1, "selectionIndex": 2}]
        }));
        assert!(snapshot.validate().is_ok());
        let models = snapshot.into_models();
        let quest = &models.quests.active_quests[0];
        assert_eq!(quest.accept_npc_index, Some(0));
        assert_eq!(quest.finish_npc_index, None);
        assert_eq!(quest.status, QuestStatus::ReadyToTurnIn);
        assert!(matches!(&quest.rewards[0], QuestReward::Item {
            selection_index: Some(2), item_id, ..
        } if item_id == "iron-sword"));
        assert_eq!(models.read_model.player.level, 4);
    }

    #[test]
    fn reward_preview_retains_original_item_info_and_rejects_invalid_widths() {
        let raw = json!({
            "type": "item", "itemId": "973", "name": "Fencing", "quantity": 1,
            "icon": 3640,
            "tooltipSource": {
                "info": {"item_index": 973, "name": "Fencing", "image": 3640,
                    "durability": 1000},
                "userItem": {"item_index": 973, "current_dura": 1000,
                    "max_dura": 1000, "count": 0, "identified": false}
            }
        });
        let reward: QuestReward = serde_json::from_value::<WebQuestReward>(raw.clone())
            .expect("source-shaped preview")
            .into();
        let QuestReward::Item {
            tooltip_source: Some(source),
            ..
        } = reward
        else {
            panic!("preview source was discarded");
        };
        assert_eq!(source.info.item_index, 973);
        assert_eq!(source.info.image, 3640);
        let user = source.user_item.expect("preview UserItem");
        assert_eq!(user.count, 0);
        assert_eq!(user.current_dura, 1000);
        assert!(!user.identified);
        let mut invalid = raw;
        invalid["tooltipSource"]["info"]["image"] = json!(65536);
        assert!(serde_json::from_value::<WebQuestReward>(invalid).is_err());
    }

    #[test]
    fn duplicate_quest_and_missing_open_npc_are_rejected() {
        let quest = json!({"questIndex": 7, "title": "A Quest", "status": "inProgress"});
        let mut snapshot = snapshot_with(quest.clone());
        snapshot.quests.push(serde_json::from_value(quest).unwrap());
        assert_eq!(snapshot.validate(), Err("invalid or duplicate quest index"));
        snapshot.quests.pop();
        snapshot.dialog.is_open = true;
        assert_eq!(snapshot.validate(), Err("open dialog lacks NPC identity"));
    }

    #[test]
    fn ack_requires_exact_operation_fields() {
        let ack: WebQuestAck = serde_json::from_value(json!({
            "generation": 1, "operation": "finishQuest", "requestId": "qs-1",
            "questIndex": 7, "selectedItemIndex": 2, "success": false
        }))
        .unwrap();
        assert!(matches!(
            ack.clone().into_shared(),
            Some(
                mir2_client_bevy::pending_operations::QuestOperationAck::FinishQuest {
                    quest_index: 7,
                    selected_item_index: 2,
                    success: false,
                    ..
                }
            )
        ));
        let mut wrong = ack;
        wrong.npc_index = Some(9);
        assert!(wrong.into_shared().is_none());
    }

    #[test]
    fn presentation_accepts_css_scale_without_confusing_it_with_device_pixels() {
        let metrics: WebQuestUiPresentation = serde_json::from_value(json!({
            "logicalWidth": 1024.0, "logicalHeight": 768.0,
            "stageCssScale": 0.5078125, "touch": true
        }))
        .unwrap();
        assert!(metrics.matches_window(1024.0, 768.0));
        let mut resolution = bevy::window::WindowResolution::new(2048, 1536);
        resolution.set_scale_factor(2.0);
        assert!(metrics.matches_window(resolution.width(), resolution.height()));
        resolution.set_physical_resolution(1024, 768);
        assert!(!metrics.matches_window(resolution.width(), resolution.height()));
        assert!(!metrics.matches_window(520.0, 390.0));
        assert!(!metrics.matches_window(2048.0, 1536.0));
        assert!(metrics.touch);
    }

    #[test]
    fn resized_or_invalid_presentation_cannot_take_shared_input() {
        let mut metrics = WebQuestUiPresentation {
            logical_width: 1024.0,
            logical_height: 768.0,
            stage_css_scale: 0.5078125,
            touch: true,
        };
        assert!(!metrics.matches_window(1200.0, 768.0));
        metrics.logical_width = 1200.0;
        assert!(metrics.matches_window(1200.0, 768.0));
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY, 17.0] {
            metrics.stage_css_scale = scale;
            assert!(!metrics.matches_window(1200.0, 768.0));
        }
        metrics.stage_css_scale = 1.0;
        for width in [0.0, f32::NAN, f32::INFINITY, 16_385.0] {
            metrics.logical_width = width;
            assert!(!metrics.matches_window(width, 768.0));
        }
    }

    #[test]
    fn unsupported_touch_viewports_do_not_silently_use_desktop_buttons() {
        let mut metrics = WebQuestUiPresentation {
            logical_width: 1365.0,
            logical_height: 768.0,
            stage_css_scale: 0.46875,
            touch: true,
        };
        assert!(
            metrics.for_window(1365.0, 768.0).is_some(),
            "640x360 landscape fits"
        );
        // The actual 640x360 browser stage uses an integer virtual width,
        // giving a 359.298 CSS px height after uniform scaling.
        metrics.logical_width = 1368.0;
        metrics.stage_css_scale = 640.0 / 1368.0;
        assert!(metrics.for_window(1368.0, 768.0).is_some());
        metrics.logical_width = 1365.0;
        metrics.stage_css_scale = 0.4;
        assert!(
            metrics.for_window(1365.0, 768.0).is_none(),
            "short touch viewport needs compatibility UI"
        );
        metrics.touch = false;
        assert!(
            metrics.for_window(1365.0, 768.0).is_some(),
            "desktop source layout is unchanged"
        );
    }

    #[test]
    fn quest_name_targets_are_native_tracker_projection_with_current_full_stamp() {
        let mut world=serde_json::from_value::<WebQuestWorldContext>(quest_world_v2_json()).unwrap().into_resource();
        let stamp=world.stamp.clone().unwrap(); let tracker=quest_world_hunt_tracker();
        let host=QuestUiHostContext { generation:stamp.generation,revision:stamp.revision,in_game:true,host_visible:true,..Default::default() };
        let actual=QuestNameTargets::from_context(&world,&host,&tracker,true);
        let expected:Vec<u32>=world.raw_entities.iter().filter(|row| row.kind==mir2_client_bevy::entities::EntityKind::Monster && row.dead==Some(false)
            && mir2_client_bevy::crystal_ui::quest_targets::tracker_targets_monster(&tracker,&row.name)).map(|row| row.object_id).collect();
        assert!(actual.known); assert_eq!(actual.object_ids,expected); assert!(actual.object_ids.contains(&44));
        assert_eq!(actual.stamp.as_ref(),Some(&stamp));
        let json=serde_json::to_value(&actual).unwrap(); assert_eq!(json["version"],1);
        assert_eq!(json["stamp"]["sceneRevision"],stamp.scene_revision);
        world.raw_entities.iter_mut().find(|row|row.object_id==44).unwrap().dead=Some(true);
        assert!(!QuestNameTargets::from_context(&world,&host,&tracker,true).object_ids.contains(&44));
    }
    #[test]
    fn quest_name_targets_missing_context_stale_host_or_incomplete_quest_are_unknown_or_empty() {
        let world=serde_json::from_value::<WebQuestWorldContext>(quest_world_v2_json()).unwrap().into_resource();
        let stamp=world.stamp.clone().unwrap(); let mut tracker=quest_world_hunt_tracker();
        let mut host=QuestUiHostContext { generation:stamp.generation,revision:stamp.revision,in_game:true,host_visible:true,..Default::default() };
        assert!(!QuestNameTargets::from_context(&world,&host,&tracker,false).known);
        host.revision+=1; let stale=QuestNameTargets::from_context(&world,&host,&tracker,true);
        assert!(!stale.known); assert!(stale.stamp.is_none()); assert!(stale.object_ids.is_empty());
        host.revision=stamp.revision; tracker.active_quests[0].objectives[0].current=2;
        let complete=QuestNameTargets::from_context(&world,&host,&tracker,true); assert!(complete.known); assert!(complete.object_ids.is_empty());
        host.host_visible=false; assert!(!QuestNameTargets::from_context(&world,&host,&tracker,true).known);
        assert!(!QuestNameTargets::from_context(&Default::default(),&host,&tracker,true).known);
    }
}
