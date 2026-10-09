//! Browser-capable host adapter for the shared Bevy Quest UI tree.
//! The actual Diary, detail, NPC buttons, and intent queue live in `quest_ui`.

use bevy::prelude::*;
use bevy::text::FontSource;
use mir2_ui_core::action::UiAction;

use crate::crystal_ui::overlays::{dispatch_ui_action, NativePlayerUiState, UiEffectQueue};
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::quest_model::NpcDialogModel;
use crate::quest_ui::{Mir2QuestUiPlugin, QuestUiIntent, QuestUiIntentQueue, QuestUiState};
use crate::{inventory::InventoryModel, read_model::PlayerStats, skill_model::SkillModel};
use crate::entities::{EntityKind, EntityModel, EntityModelSet};
use crate::quest_model::{CombatTarget, CombatTargetModel, GroundPickupModel, RecentPickup, QuestTracker};
use crate::quest_ui::QuestRouteNavigationIntentQueue;

/// Raw Quest facts retain unknown life/HP values; projections never prove life.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestWorldEntity {
    pub object_id: u32,
    pub kind: EntityKind,
    pub name: String,
    pub x: i32,
    pub y: i32,
    #[serde(deserialize_with = "required_nullable")]
    pub direction: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub level: Option<u32>,
    #[serde(deserialize_with = "required_nullable")]
    pub dead: Option<bool>,
    #[serde(deserialize_with = "required_nullable")]
    pub hp: Option<i32>,
    #[serde(deserialize_with = "required_nullable")]
    pub max_hp: Option<i32>,
}

pub fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where D: serde::Deserializer<'de>, T: serde::Deserialize<'de> {
    <Option<T> as serde::Deserialize<'de>>::deserialize(deserializer)
}

impl QuestWorldEntity {
    pub fn living_monster(&self) -> bool {
        self.kind == EntityKind::Monster && self.dead == Some(false)
            && self.hp.is_none_or(|hp| hp > 0)
            && self.max_hp.is_none_or(|max| max > 0 && self.hp.is_some_and(|hp| hp > 0))
    }
    fn model(&self) -> EntityModel {
        EntityModel { object_id: self.object_id.to_string(), kind: self.kind,
            name: self.name.clone(), x: self.x, y: self.y,
            direction: self.direction.clone(), level: self.level }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestGroundDrop {
    pub object_id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub quantity: u32,
    pub source_monster: String,
}

/// These Crystal sprites must be present before the host reports visual
/// readiness. The runtime may load more assets as panels become visible.
pub use crate::quest_ui::PORTABLE_QUEST_REQUIRED_SKINS;

/// The browser owns session visibility and shortcuts. `open_revision` is
/// incremented only for an authoritative external open/close request; local
/// Quest button and keyboard changes survive ordinary snapshot refreshes.
#[derive(Debug, Clone, Resource)]
pub struct QuestUiHostContext {
    pub generation: u64,
    pub revision: u64,
    pub host_visible: bool,
    /// Allows the compact tree to complete UI layout while input remains blocked.
    pub layout_preparing: bool,
    pub in_game: bool,
    pub quest_log_open: bool,
    pub open_revision: u64,
    pub blocks_gameplay_keys: bool,
    pub turn_in_blocked: bool,
    pub quest_toggle_pressed: bool,
    pub quest_close_pressed: bool,
}

impl Default for QuestUiHostContext {
    fn default() -> Self {
        Self {
            generation: 0,
            revision: 0,
            host_visible: false,
            layout_preparing: false,
            in_game: false,
            quest_log_open: false,
            open_revision: 0,
            blocks_gameplay_keys: false,
            turn_in_blocked: false,
            quest_toggle_pressed: false,
            quest_close_pressed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestWorldStamp {
    pub generation: u64,
    pub revision: u64,
    pub connection_generation: u64,
    pub session_generation: u64,
    pub scene_revision: u64,
    pub player_object_id: u32,
    pub map_file_name: String,
}

/// Quest-only authoritative supply inputs. The Bag and HUD read models have
/// independent owners and cannot fill a missing Quest context.
#[derive(Resource, Default)]
pub struct QuestWorldContext {
    pub stamp: Option<QuestWorldStamp>,
    pub inventory: Option<InventoryModel>,
    pub player: Option<PlayerStats>,
    pub skills: Option<SkillModel>,
    pub map_index: Option<i32>,
    pub raw_entities: Vec<QuestWorldEntity>,
    pub selected_object_id: Option<u32>,
    pub ground_drops: Vec<QuestGroundDrop>,
    pub map: Option<crate::map::MapModel>,
    pub big_map: Option<crate::big_map::BigMapModel>,
    pub entities: EntityModelSet,
    pub guidance_entities: EntityModelSet,
    pub target: CombatTargetModel,
    pub pickups: GroundPickupModel,
}

impl QuestWorldContext {
    pub fn clear(&mut self) { *self = Self::default(); }
    pub fn current(&self, host: &QuestUiHostContext) -> bool {
        host.in_game && host.host_visible && self.stamp.as_ref().is_some_and(|stamp|
            stamp.generation == host.generation && stamp.revision == host.revision)
            && self.inventory.is_some() && self.player.is_some()
    }
    pub fn self_tile(&self) -> Option<(i32, i32)> {
        let id = self.stamp.as_ref()?.player_object_id;
        self.raw_entities.iter().find(|row| row.kind == EntityKind::SelfPlayer && row.object_id == id)
            .map(|row| (row.x, row.y))
    }
    pub fn rebuild_projections(&mut self) {
        self.entities.entities = self.raw_entities.iter().map(QuestWorldEntity::model).collect();
        self.guidance_entities.entities = self.raw_entities.iter()
            .filter(|row| row.kind != EntityKind::Monster || row.living_monster())
            .map(QuestWorldEntity::model).collect();
        let tile = self.self_tile();
        self.map = tile.map(|(x, y)| crate::map::MapModel { center_x: x, center_y: y, ..Default::default() });
        self.big_map = self.stamp.as_ref().map(|stamp| crate::big_map::BigMapModel {
            reset_epoch: stamp.scene_revision, current_map_index: self.map_index,
            player_location: tile.map(|(x, y)| crate::big_map::BigMapPoint { x, y }),
            ..Default::default()
        });
        self.target.target = self.selected_object_id.and_then(|id| self.raw_entities.iter()
            .find(|row| row.object_id == id && row.living_monster()))
            .and_then(|row| Some(CombatTarget { object_id: row.object_id, name: row.name.clone(),
                hp: row.hp?, max_hp: row.max_hp?, is_player: false }));
        self.pickups.recent = tile.map(|(x, y)| {
            let mut rows = self.ground_drops.iter().collect::<Vec<_>>();
            rows.sort_by_key(|row| (x.abs_diff(row.x).max(y.abs_diff(row.y)), row.object_id));
            rows.into_iter().take(4).map(|row| RecentPickup { object_id: Some(row.object_id),
                key: row.object_id.to_string(), label: row.name.clone(), amount: row.quantity,
                from_npc: Some(row.source_monster.clone()) }).collect()
        }).unwrap_or_default();
    }
    pub fn attack_current(&self, tracker: &QuestTracker, object_id: u32, quest: bool) -> bool {
        self.raw_entities.iter().any(|row| row.object_id == object_id && row.living_monster())
            && if quest {
                crate::quest_ui::quest_target_is_visible(tracker, &self.entities, object_id)
            } else {
                self.selected_object_id == Some(object_id)
                    && crate::quest_ui::target_is_attackable(Some(&self.target), object_id)
            }
    }
    pub fn drop_current(&self, object_id: u32, x: i32, y: i32) -> bool {
        self.ground_drops.iter().any(|row| row.object_id == object_id && row.x == x && row.y == y)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestPresentationAction {
    OpenDestinationMap(QuestWorldStamp),
    AttackTarget { stamp: QuestWorldStamp, object_id: u32 },
    AttackQuestTarget { stamp: QuestWorldStamp, object_id: u32 },
    PickUpObject { stamp: QuestWorldStamp, object_id: u32, x: i32, y: i32 },
    PickUpTile { stamp: QuestWorldStamp, x: i32, y: i32 },
}

impl QuestPresentationAction {
    pub fn stamp(&self) -> &QuestWorldStamp {
        match self {
            Self::OpenDestinationMap(stamp) | Self::AttackTarget { stamp, .. }
            | Self::AttackQuestTarget { stamp, .. } | Self::PickUpObject { stamp, .. }
            | Self::PickUpTile { stamp, .. } => stamp,
        }
    }
    pub fn current(&self, world: &QuestWorldContext, tracker: &QuestTracker) -> bool {
        world.stamp.as_ref() == Some(self.stamp()) && match *self {
            Self::OpenDestinationMap(_) => world.inventory.is_some() && world.player.is_some(),
            Self::AttackTarget { object_id, .. } => world.attack_current(tracker, object_id, false),
            Self::AttackQuestTarget { object_id, .. } => world.attack_current(tracker, object_id, true),
            Self::PickUpObject { object_id, x, y, .. } => world.drop_current(object_id, x, y),
            Self::PickUpTile { x, y, .. } => world.self_tile() == Some((x, y)),
        }
    }
}

#[derive(Resource, Default)]
pub struct QuestRouteWorldStamp(pub Option<QuestWorldStamp>);

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestWorldControlRect {
    #[serde(rename = "type")]
    pub action_type: &'static str,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Resource, Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestWorldControlRects {
    pub version: u8,
    pub stamp: Option<QuestWorldStamp>,
    pub logical_width: f32,
    pub logical_height: f32,
    pub rects: Vec<QuestWorldControlRect>,
}

impl Default for QuestWorldControlRects {
    fn default() -> Self { Self { version: 1, stamp: None, logical_width: 0.0,
        logical_height: 0.0, rects: Vec::new() } }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestWorldControlObservationSet { Observe }

#[derive(Resource, Default)]
pub struct QuestPresentationActionQueue(Option<QuestPresentationAction>);

impl QuestPresentationActionQueue {
    pub fn push(&mut self, action: QuestPresentationAction) -> bool {
        if self.0.is_some() { return false; }
        self.0 = Some(action);
        true
    }
    pub fn take(&mut self) -> Option<QuestPresentationAction> { self.0.take() }
    pub fn clear(&mut self) { self.0 = None; }
}

/// A packaged font supplied by the browser runtime. No system-family or
/// default-font fallback is used for the portable Quest tree.
#[derive(Resource, Clone)]
pub struct QuestUiFont(pub Handle<Font>);

/// Insert before the shared Quest Startup system runs. Missing target keeps
/// the tree unspawned rather than routing UI to the world canvas.
#[derive(Resource, Clone, Copy)]
pub struct QuestUiTargetCamera(pub Entity);

#[derive(Default, Resource)]
struct AppliedOpenRevision(Option<(u64, u64)>);

pub struct Mir2PortableQuestUiPlugin;

impl Plugin for Mir2PortableQuestUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestUiHostContext>()
            .init_resource::<QuestWorldContext>()
            .init_resource::<QuestPresentationActionQueue>()
            .init_resource::<QuestRouteWorldStamp>()
            .init_resource::<QuestWorldControlRects>()
            .init_resource::<AppliedOpenRevision>()
            .init_resource::<NativeShellModel>()
            .add_plugins(Mir2QuestUiPlugin)
            .add_systems(
                Update,
                apply_host_context
                    .after(crate::pending_operations::PendingLifecycleSet::Ingest)
                    .before(crate::quest_ui::process_quest_ui_input),
            )
            .add_systems(
                PostUpdate,
                apply_packaged_quest_font.before(bevy::ui::UiSystems::Layout),
            );
        #[cfg(not(feature = "native-ui"))]
        app.add_systems(PostUpdate, observe_world_control_rects
            .after(bevy::ui::UiSystems::Layout).in_set(QuestWorldControlObservationSet::Observe));
    }
}

fn measured_control_rect(action_type: &'static str, size: Vec2, center: Vec2,
    logical_width: f32, logical_height: f32) -> Option<QuestWorldControlRect> {
    let left = center.x - size.x * 0.5;
    let top = center.y - size.y * 0.5;
    if ![size.x, size.y, left, top, logical_width, logical_height].into_iter().all(f32::is_finite)
        || size.min_element() <= 0.0 || left < 0.0 || top < 0.0
        || left + size.x > logical_width + 1.0 || top + size.y > logical_height + 1.0 { return None; }
    Some(QuestWorldControlRect { action_type, left, top, width: size.x, height: size.y })
}

#[cfg(not(feature = "native-ui"))]
fn observe_world_control_rects(
    host: Res<QuestUiHostContext>, world: Res<QuestWorldContext>,
    presentation: Option<Res<crate::quest_ui::QuestUiPresentation>>,
    target: Res<QuestUiTargetCamera>,
    state: Res<QuestUiState>, tracker: Res<QuestTracker>,
    journey_models: (Res<crate::quest_guidance::QuestGuidance>, Res<crate::quest_journey::NewcomerJourneyCatalog>,
        Res<crate::quest_model::CompletedQuestTracker>, Res<crate::read_model::UiReadModel>),
    roots: Query<(Entity, &ComputedNode, &InheritedVisibility, &bevy::ui::UiTargetCamera), With<crate::quest_ui::QuestUiRoot>>,
    controls: Query<(Entity, &crate::quest_ui::QuestUiButton, &ComputedNode, &bevy::ui::UiGlobalTransform,
        &InheritedVisibility), With<Button>>,
    ancestors: Query<(Option<&ChildOf>, Option<&Node>, Option<&InheritedVisibility>)>,
    mut observed: ResMut<QuestWorldControlRects>,
) {
    *observed = QuestWorldControlRects::default();
    if !world.current(&host) || world.self_tile().is_none() { return; }
    let Some(presentation) = presentation.as_deref().filter(|presentation| presentation.supports_world_tracker()) else { return; };
    let Ok((root, root_node, visible, camera)) = roots.single() else { return; };
    let root_size = root_node.size() * root_node.inverse_scale_factor;
    if !visible.get() || !root_size.is_finite() || root_size.min_element() <= 0.0 || camera.0 != target.0
        || (root_size.x - presentation.logical_width).abs() > 1.0
        || (root_size.y - presentation.logical_height).abs() > 1.0 { return; }
    let (guidance, catalog, completed, read_model) = journey_models;
    let journey = catalog.derive(&guidance, &tracker, &completed, &read_model.player);
    observed.stamp = world.stamp.clone();
    observed.logical_width = presentation.logical_width;
    observed.logical_height = presentation.logical_height;
    for (entity, action, node, transform, visible) in &controls {
        use crate::quest_ui::QuestUiButton as ButtonAction;
        if !visible.get() { continue; }
        let action_type = match action {
            ButtonAction::NavigateQuestRoute(intent) if crate::quest_ui::quest_route_intent_is_current(
                *intent, &tracker, &state, journey.as_ref(), world.big_map.as_ref()) => "navigateQuestRoute",
            ButtonAction::AttackTarget { .. } if crate::quest_ui::portable_world_action(action, &world, &tracker).is_some() => "attackTarget",
            ButtonAction::AttackQuestTarget { .. } if crate::quest_ui::portable_world_action(action, &world, &tracker).is_some() => "attackQuestTarget",
            ButtonAction::PickUpObject { .. } if crate::quest_ui::portable_world_action(action, &world, &tracker).is_some() => "pickUpObject",
            ButtonAction::PickUpTile if crate::quest_ui::portable_world_action(action, &world, &tracker).is_some() => "pickUpTile",
            ButtonAction::ToggleSupplies => "toggleSupplies",
            ButtonAction::SelectSupplyVendor(_) => "selectSupplyVendor",
            ButtonAction::ShowSupplyInventory => "showSupplyInventory",
            ButtonAction::OpenDestinationMap => "openDestinationMap",
            _ => continue,
        };
        let mut ancestor = entity;
        let mut reaches_root = false;
        for _ in 0..32 {
            let Ok((parent, node, visibility)) = ancestors.get(ancestor) else { break; };
            if node.is_some_and(|node| node.display == Display::None) || visibility.is_some_and(|visible| !visible.get()) { break; }
            if ancestor == root { reaches_root = true; break; }
            let Some(parent) = parent else { break; };
            ancestor = parent.parent();
        }
        if !reaches_root { continue; }
        let scale = node.inverse_scale_factor;
        let Some(rect) = measured_control_rect(action_type, node.size() * scale,
            transform.affine().translation * scale, presentation.logical_width, presentation.logical_height) else { continue; };
        if observed.rects.len() >= 128 { *observed = QuestWorldControlRects::default(); return; }
        observed.rects.push(rect);
    }
}

fn apply_host_context(
    mut context: ResMut<QuestUiHostContext>,
    mut revision: ResMut<AppliedOpenRevision>,
    mut shell: ResMut<NativeShellModel>,
    mut player_ui: ResMut<NativePlayerUiState>,
    mut effects: ResMut<UiEffectQueue>,
    mut queue: ResMut<QuestUiIntentQueue>,
    mut dialog: ResMut<NpcDialogModel>,
    mut quest_state: ResMut<QuestUiState>,
    world_actions: (Res<QuestWorldContext>, ResMut<QuestPresentationActionQueue>,
        ResMut<QuestRouteNavigationIntentQueue>, ResMut<QuestRouteWorldStamp>),
) {
    let (world, mut actions, mut routes, mut route_stamp) = world_actions;
    if !world.current(&context) {
        actions.clear();
        routes.clear();
        route_stamp.0 = None;
    } else {
        if actions.0.as_ref().is_some_and(|action| world.stamp.as_ref() != Some(action.stamp())) {
            actions.clear();
        }
        if route_stamp.0.as_ref().is_some_and(|stamp| world.stamp.as_ref() != Some(stamp)) {
            routes.clear();
            route_stamp.0 = None;
        }
    }
    // A surface can temporarily lose ownership while its new CSS geometry is
    // acknowledged. That is not a character/session exit: keep selected detail,
    // scroll and pending authority. The shared input/render gates separately
    // suppress this surface while it is hidden.
    let screen = if context.in_game {
        NativeShellScreen::InGame
    } else {
        NativeShellScreen::Connecting
    };
    if shell.screen != screen { shell.screen = screen; }
    if player_ui.blocks_gameplay != context.blocks_gameplay_keys {
        player_ui.blocks_gameplay = context.blocks_gameplay_keys;
    }
    if player_ui.blocks_world != context.turn_in_blocked {
        player_ui.blocks_world = context.turn_in_blocked;
    }
    if revision.0.is_none_or(|(generation, seen)|
        context.generation != generation || context.open_revision > seen
    ) {
        revision.0 = Some((context.generation, context.open_revision));
        let action = if context.quest_log_open { UiAction::OpenQuestLog } else { UiAction::ClosePanel };
        if context.quest_log_open || player_ui.quest_open() {
            dispatch_ui_action(&mut player_ui.core, &mut effects, action);
        }
    }
    if !context.host_visible || !context.in_game {
        // Host key edges must never be replayed when a hidden surface returns.
        context.quest_toggle_pressed = false;
        context.quest_close_pressed = false;
        return;
    }
    if context.quest_toggle_pressed {
        let action = if player_ui.quest_open() { UiAction::ClosePanel } else { UiAction::OpenQuestLog };
        dispatch_ui_action(&mut player_ui.core, &mut effects, action);
        context.quest_toggle_pressed = false;
    }
    if context.quest_close_pressed {
        if player_ui.quest_open() {
            dispatch_ui_action(&mut player_ui.core, &mut effects, UiAction::ClosePanel);
            quest_state.clear_diary_selection();
        } else if dialog.is_open && queue.push_intent(QuestUiIntent::SelectNpcDialog { target: "@Exit".to_owned() }) {
            dialog.close();
        }
        context.quest_close_pressed = false;
    }
}

fn apply_packaged_quest_font(
    font: Option<Res<QuestUiFont>>,
    roots: Query<Entity, With<crate::quest_ui::QuestUiRoot>>,
    parents: Query<&ChildOf>,
    mut texts: Query<(Entity, &mut TextFont)>,
) {
    let Some(font) = font else { return; };
    for (entity, mut text_font) in texts.iter_mut() {
        let mut current = entity;
        let mut belongs_to_quest = false;
        for _ in 0..24 {
            if roots.get(current).is_ok() { belongs_to_quest = true; break; }
            let Ok(parent) = parents.get(current) else { break; };
            current = parent.parent();
        }
        if belongs_to_quest {
            let source = FontSource::Handle(font.0.clone());
            if text_font.font != source { text_font.font = source; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest_model::{Quest, QuestStatus, QuestTracker};
    use crate::quest_ui::QuestUiButton;

    fn world_fixture() -> QuestWorldContext {
        let stamp = QuestWorldStamp { generation: 7, revision: 9,
            connection_generation: 2, session_generation: 3, scene_revision: 4,
            player_object_id: 1000, map_file_name: "0".into() };
        let self_player = QuestWorldEntity { object_id: 1000, kind: EntityKind::SelfPlayer,
            name: "Player".into(), x: 100, y: 200, direction: None, level: Some(5),
            dead: None, hp: None, max_hp: None };
        let monster = QuestWorldEntity { object_id: 44, kind: EntityKind::Monster,
            name: "Hen".into(), x: 102, y: 200, direction: None, level: None,
            dead: Some(false), hp: Some(12), max_hp: Some(20) };
        let mut world = QuestWorldContext { stamp: Some(stamp),
            inventory: Some(InventoryModel::default()), player: Some(PlayerStats { level: 5, ..Default::default() }),
            raw_entities: vec![self_player, monster], selected_object_id: Some(44),
            ground_drops: vec![QuestGroundDrop { object_id: 80, name: "Gold".into(), x: 102, y: 201,
                quantity: 2, source_monster: "Hen".into() }], ..Default::default() };
        world.rebuild_projections();
        world
    }

    fn world_app() -> App {
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext { generation: 7, revision: 9,
            host_visible: true, in_game: true, quest_log_open: true, open_revision: 1,
            ..Default::default() });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.insert_resource(world_fixture());
        let mut hunt = quest(23);
        hunt.status = QuestStatus::InProgress;
        hunt.objectives.push(crate::quest_model::QuestObjective { objective_id: "hen".into(),
            text: "Defeat Hen".into(), current: 0, target: 2 });
        app.insert_resource(QuestTracker { active_quests: vec![hunt] });
        app.update();
        app
    }

    #[test]
    fn quest_world_control_rects_keep_fractional_measured_geometry_and_reject_unknown_bounds() {
        let measured = measured_control_rect("attackQuestTarget", Vec2::new(80.5, 30.25),
            Vec2::new(140.25, 85.125), 800.0, 600.0).unwrap();
        assert_eq!((measured.left, measured.top, measured.width, measured.height), (100.0, 70.0, 80.5, 30.25));
        assert!(measured_control_rect("attackTarget", Vec2::ZERO, Vec2::ZERO, 800.0, 600.0).is_none());
        assert!(measured_control_rect("attackTarget", Vec2::new(20.0, 20.0), Vec2::ZERO, 800.0, 600.0).is_none());
        assert!(measured_control_rect("attackTarget", Vec2::new(20.0, 20.0), Vec2::new(900.0, 30.0), 800.0, 600.0).is_none());
        assert!(measured_control_rect("attackTarget", Vec2::new(f32::NAN, 20.0), Vec2::new(40.0, 40.0), 800.0, 600.0).is_none());
        let empty = serde_json::to_value(QuestWorldControlRects::default()).unwrap();
        assert_eq!(empty["version"], 1);
        assert_eq!(empty["stamp"], serde_json::Value::Null);
        assert_eq!(empty["rects"], serde_json::json!([]));
    }

    #[cfg(not(feature = "native-ui"))]
    #[test]
    fn portable_tracker_requires_current_quest_authority_and_hides_missing_context() {
        let mut app = world_app();
        let displayed = |app: &mut App| app.world_mut().query_filtered::<&Node,
            With<crate::quest_ui::QuestTrackerPanel>>().single(app.world()).unwrap().display;
        assert_eq!(displayed(&mut app), Display::Flex);
        app.update(); // Complete initial resource change detection.
        let buttons = |app: &mut App| app.world_mut().query_filtered::<Entity, With<Button>>()
            .iter(app.world()).collect::<std::collections::HashSet<_>>();
        let before = buttons(&mut app);
        assert!(!before.is_empty());
        // Runtime visibility synchronization can write identical values. Such
        // a frame must preserve measured nodes and any ordinary pointer edge.
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
        app.update();
        assert_eq!(buttons(&mut app), before);
        app.world_mut().resource_mut::<QuestWorldContext>().clear();
        app.update();
        assert_eq!(displayed(&mut app), Display::None);
        app.insert_resource(world_fixture());
        app.update();
        assert_eq!(displayed(&mut app), Display::Flex);
        app.world_mut().resource_mut::<QuestUiHostContext>().revision += 1;
        app.update();
        assert_eq!(displayed(&mut app), Display::None);
    }

    #[test]
    fn quest_world_raw_life_unknown_map_and_pickup_coordinates_stay_authoritative() {
        let mut world = world_fixture();
        assert_eq!(world.self_tile(), Some((100, 200)));
        assert_eq!(world.big_map.as_ref().unwrap().current_map_index, None);
        assert_eq!(world.big_map.as_ref().unwrap().reset_epoch, 4);
        assert_eq!(world.map.as_ref().unwrap().center_x, 100);
        assert_eq!(world.pickups.recent.front().unwrap().object_id, Some(80));
        assert!(world.drop_current(80, 102, 201));
        assert!(!world.drop_current(80, 100, 200));
        let tracker = QuestTracker::default();
        assert!(world.attack_current(&tracker, 44, false));
        world.raw_entities[1].dead = None;
        world.rebuild_projections();
        assert!(!world.attack_current(&tracker, 44, false));
        assert!(world.target.target.is_none());
        assert!(!world.guidance_entities.entities.iter().any(|row| row.kind == EntityKind::Monster));
        world.raw_entities[1].dead = Some(false);
        world.raw_entities[1].hp = None;
        world.rebuild_projections();
        assert!(!world.raw_entities[1].living_monster());
        world.raw_entities[1].max_hp = None;
        world.rebuild_projections();
        assert!(world.raw_entities[1].living_monster());
        assert!(world.target.target.is_none(), "unknown HP cannot prove an ordinary target");
        world.raw_entities.remove(0);
        world.rebuild_projections();
        assert!(world.self_tile().is_none());
        assert!(world.map.is_none());
        assert!(world.pickups.recent.is_empty());
    }

    #[cfg(not(feature = "native-ui"))]
    #[test]
    fn portable_world_buttons_preserve_origin_and_coordinates_without_wire_intents() {
        let mut app = world_app();
        let cases = [
            QuestUiButton::AttackTarget { object_id: 44 },
            QuestUiButton::AttackQuestTarget { object_id: 44 },
            QuestUiButton::PickUpObject { object_id: 80 }, QuestUiButton::PickUpTile,
        ];
        for button in cases {
            let entity = app.world_mut().spawn((Button, Interaction::Pressed, button.clone())).id();
            app.update();
            let action = app.world_mut().resource_mut::<QuestPresentationActionQueue>().take().expect("current local action");
            match button {
                QuestUiButton::AttackTarget { .. } => assert!(matches!(action, QuestPresentationAction::AttackTarget { object_id: 44, .. })),
                QuestUiButton::AttackQuestTarget { .. } => assert!(matches!(action, QuestPresentationAction::AttackQuestTarget { object_id: 44, .. })),
                QuestUiButton::PickUpObject { .. } => assert!(matches!(action, QuestPresentationAction::PickUpObject { object_id: 80, x: 102, y: 201, .. })),
                QuestUiButton::PickUpTile => assert!(matches!(action, QuestPresentationAction::PickUpTile { x: 100, y: 200, .. })),
                _ => unreachable!(),
            }
            assert!(app.world_mut().resource_mut::<QuestPresentationActionQueue>().take().is_none());
            assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());
            app.world_mut().despawn(entity);
        }
        app.world_mut().resource_mut::<QuestWorldContext>().selected_object_id = Some(45);
        app.world_mut().spawn((Button, Interaction::Pressed, QuestUiButton::AttackTarget { object_id: 44 }));
        app.update();
        assert!(app.world_mut().resource_mut::<QuestPresentationActionQueue>().take().is_none());
        app.world_mut().resource_mut::<QuestWorldContext>().raw_entities[1].dead = Some(true);
        app.world_mut().spawn((Button, Interaction::Pressed, QuestUiButton::AttackQuestTarget { object_id: 44 }));
        app.update();
        assert!(app.world_mut().resource_mut::<QuestPresentationActionQueue>().take().is_none());
    }

    #[cfg(not(feature = "native-ui"))]
    #[test]
    fn portable_route_is_stamped_single_slot_and_hidden_or_changed_authority_clears_actions() {
        use crate::quest_supplies::SupplyVendor;
        use crate::quest_ui::{QuestRouteNavigationIntent, QuestRouteTarget};
        let mut app = world_app();
        let destination = SupplyVendor::Potions.destination().unwrap();
        {
            let mut world = app.world_mut().resource_mut::<QuestWorldContext>();
            world.map_index = Some(destination.map_index);
            world.rebuild_projections();
        }
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.supply_open = true; state.supply_vendor = Some(SupplyVendor::Potions);
        }
        let intent = QuestRouteNavigationIntent { target: QuestRouteTarget::Supply { vendor: SupplyVendor::Potions },
            quest_index: 0, reset_epoch: 4, map_index: destination.map_index, x: destination.x, y: destination.y };
        app.world_mut().spawn((Button, Interaction::Pressed, QuestUiButton::NavigateQuestRoute(intent)));
        app.update();
        assert_eq!(app.world().resource::<QuestRouteWorldStamp>().0.as_ref().unwrap().revision, 9);
        app.world_mut().spawn((Button, Interaction::Pressed, QuestUiButton::NavigateQuestRoute(intent)));
        app.update();
        assert_eq!(app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().take(), Some(intent));
        assert!(app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().take().is_none());
        for change in 0..3 {
            let stamp = app.world().resource::<QuestWorldContext>().stamp.clone().unwrap();
            app.world_mut().resource_mut::<QuestPresentationActionQueue>().push(QuestPresentationAction::OpenDestinationMap(stamp.clone()));
            app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().push(intent);
            app.world_mut().resource_mut::<QuestRouteWorldStamp>().0 = Some(stamp);
            match change {
                0 => app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = false,
                1 => app.world_mut().resource_mut::<QuestUiHostContext>().revision = 10,
                _ => app.world_mut().resource_mut::<QuestWorldContext>().stamp.as_mut().unwrap().scene_revision += 1,
            }
            app.update();
            assert!(app.world_mut().resource_mut::<QuestPresentationActionQueue>().take().is_none());
            assert!(app.world().resource::<QuestRouteNavigationIntentQueue>().is_empty());
            assert!(app.world().resource::<QuestRouteWorldStamp>().0.is_none());
            app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
            app.world_mut().resource_mut::<QuestUiHostContext>().revision = 9;
        }
    }

    #[test]
    fn quest_world_context_requires_matching_applied_revision_and_local_map_queue_is_single_slot() {
        let stamp = QuestWorldStamp { generation: 7, revision: 9,
            connection_generation: 2, session_generation: 3, scene_revision: 4,
            player_object_id: 1000, map_file_name: "0".into() };
        let mut world = QuestWorldContext { stamp: Some(stamp.clone()),
            inventory: Some(InventoryModel { gold: 0, ..Default::default() }),
            player: Some(PlayerStats { level: 5, gold: 0, ..Default::default() }), skills: None, ..Default::default() };
        let mut host = QuestUiHostContext { generation: 7, revision: 9,
            in_game: true, host_visible: true, ..Default::default() };
        assert!(world.current(&host));
        host.revision += 1;
        assert!(!world.current(&host));
        host.revision -= 1;
        host.host_visible = false;
        assert!(!world.current(&host));
        host.host_visible = true;
        let mut queue = QuestPresentationActionQueue::default();
        assert!(queue.push(QuestPresentationAction::OpenDestinationMap(stamp.clone())));
        assert!(!queue.push(QuestPresentationAction::OpenDestinationMap(stamp.clone())));
        assert_eq!(queue.take(), Some(QuestPresentationAction::OpenDestinationMap(stamp.clone())));
        assert!(queue.take().is_none());
        assert!(queue.push(QuestPresentationAction::OpenDestinationMap(stamp)));
        queue.clear();
        world.clear();
        assert!(!world.current(&host));
        assert!(queue.take().is_none());
    }

    #[test]
    fn portable_supply_buttons_use_current_quest_context_and_map_stays_local() {
        use crate::quest_supplies::SupplyVendor;
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext { generation: 7, revision: 9,
            host_visible: true, in_game: true, quest_log_open: true, open_revision: 1,
            ..Default::default() });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.update();
        let press = |app: &mut App, action| {
            app.world_mut().spawn((Button, Interaction::Pressed, action));
            app.update();
        };
        press(&mut app, QuestUiButton::ToggleSupplies);
        assert!(!app.world().resource::<QuestUiState>().supply_open,
            "a missing complete context cannot open a fabricated supply plan");
        let stamp = QuestWorldStamp { generation: 7, revision: 9,
            connection_generation: 2, session_generation: 3, scene_revision: 4,
            player_object_id: 1000, map_file_name: "0".into() };
        app.insert_resource(QuestWorldContext { stamp: Some(stamp.clone()),
            inventory: Some(InventoryModel { gold: 0, ..Default::default() }),
            player: Some(PlayerStats { level: 5, class_name: Some("Taoist".into()),
                gender: Some("female".into()), ..Default::default() }), skills: None, ..Default::default() });
        press(&mut app, QuestUiButton::ToggleSupplies);
        assert!(app.world().resource::<QuestUiState>().supply_open);
        press(&mut app, QuestUiButton::SelectSupplyVendor(SupplyVendor::Potions));
        assert_eq!(app.world().resource::<QuestUiState>().supply_vendor, Some(SupplyVendor::Potions));
        press(&mut app, QuestUiButton::ShowSupplyInventory);
        assert_eq!(app.world().resource::<QuestUiState>().supply_vendor, None);
        press(&mut app, QuestUiButton::OpenDestinationMap);
        assert_eq!(app.world_mut().resource_mut::<QuestPresentationActionQueue>().take(),
            Some(QuestPresentationAction::OpenDestinationMap(stamp)));
        assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty(),
            "opening the browser map must not emit a Quest or world wire intent");
    }

    fn quest(index: i32) -> Quest {
        Quest {
            quest_index: index,
            accept_npc_index: Some(0),
            finish_npc_index: Some(0),
            title: "Portable quest".to_owned(),
            npc_name: None,
            group: None,
            min_level_needed: 0,
            detail: Default::default(),
            status: QuestStatus::NotStarted,
            objectives: Vec::new(),
            rewards: Vec::new(),
            unknown_text: None,
        }
    }

    #[test]
    fn shared_diary_nodes_target_the_ui_camera_and_emit_the_native_intent() {
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext {
            host_visible: true,
            in_game: true,
            quest_log_open: true,
            open_revision: 1,
            ..Default::default()
        });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.insert_resource(crate::quest_guidance::QuestGuidance::from_profile_name("newcomer-v2"));
        app.insert_resource(QuestTracker { active_quests: vec![quest(23)] });
        app.update();

        let root = app.world_mut().query_filtered::<&bevy::ui::UiTargetCamera, With<crate::quest_ui::QuestUiRoot>>()
            .single(app.world()).expect("shared Quest root");
        assert_eq!(root.0, camera);
        assert!(app.world().resource::<NativePlayerUiState>().quest_open());
        app.world_mut().resource_mut::<QuestUiState>().select_quest(23);
        app.update();
        let accept = app.world_mut().query::<(Entity, &QuestUiButton)>().iter(app.world())
            .find_map(|(entity, action)| matches!(action, QuestUiButton::AcceptQuest { npc_index: 0, quest_index: 23 }).then_some(entity))
            .expect("same Diary Accept button as native");
        app.world_mut().entity_mut(accept).insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents(),
            vec![QuestUiIntent::AcceptQuest { npc_index: 0, quest_index: 23 }]
        );
    }

    #[test]
    fn temporary_surface_handoff_preserves_detail_scroll_and_pending_without_input() {
        use crate::pending_operations::{PendingOperationKey, PendingOperations};
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext {
            generation: 7, host_visible: true, in_game: true,
            quest_log_open: true, open_revision: 1, ..Default::default()
        });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.insert_resource(crate::quest_guidance::QuestGuidance::from_profile_name("newcomer-v2"));
        app.insert_resource(QuestTracker { active_quests: vec![quest(23)] });
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.select_quest(23);
            state.detail_scroll_top = 4;
        }
        let key = PendingOperationKey::QuestAccept { npc_index: 0, quest_index: 99 };
        assert!(app.world_mut().resource_mut::<PendingOperations>().try_begin(key.clone()));
        app.update();
        let close = app.world_mut().query::<(Entity, &QuestUiButton)>().iter(app.world())
            .find_map(|(entity, action)| matches!(action, QuestUiButton::CloseQuestDetail).then_some(entity))
            .expect("rendered detail close");
        {
            let mut context = app.world_mut().resource_mut::<QuestUiHostContext>();
            context.host_visible = false;
            context.quest_close_pressed = true;
        }
        // A queued interaction from the preceding visible frame must not act
        // during geometry handoff or be re-read when the surface returns.
        app.world_mut().entity_mut(close).insert(Interaction::Pressed);
        app.update();
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.detail_quest_index, Some(23));
        assert_eq!(state.detail_scroll_top, 4);
        assert!(app.world().resource::<PendingOperations>().contains(&key));
        assert!(!app.world().resource::<QuestUiHostContext>().quest_close_pressed);
        assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());
        let display = app.world_mut().query_filtered::<&Node, With<crate::quest_ui::QuestUiRoot>>()
            .single(app.world()).unwrap().display;
        assert_eq!(display, Display::None);
        app.world_mut().resource_mut::<QuestTracker>().active_quests[0].title = "Resized authoritative quest".to_owned();
        // Let hidden rendering observe the model change before returning to
        // visible. Recovery must still rebuild its previously cached children.
        app.update();
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
        app.update();
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.detail_quest_index, Some(23));
        assert_eq!(state.detail_scroll_top, 4);
        let display = app.world_mut().query_filtered::<&Node, With<crate::quest_ui::QuestUiRoot>>()
            .single(app.world()).unwrap().display;
        assert_eq!(display, Display::Flex);
        assert!(app.world_mut().query::<&Text>().iter(app.world())
            .any(|text| text.0.contains("Resized authoritative quest")));
        assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());
        // A real session exit still resets the selected view. The runtime owns
        // pending/session-generation reset separately from presentation hiding.
        app.world_mut().resource_mut::<QuestUiHostContext>().in_game = false;
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().detail_quest_index, None);
        assert_eq!(app.world().resource::<QuestUiState>().detail_scroll_top, 0);
    }

    #[test]
    fn ingested_session_exit_applies_before_portable_context_and_input() {
        fn logout(mut context: ResMut<QuestUiHostContext>) {
            context.in_game = false;
            context.host_visible = false;
        }
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiHostContext {
            host_visible: true, in_game: true, quest_log_open: true,
            open_revision: 1, ..Default::default()
        });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.update();
        app.world_mut().resource_mut::<QuestUiState>().select_quest(23);
        app.add_systems(Update, logout.in_set(crate::pending_operations::PendingLifecycleSet::Ingest));
        app.update();
        assert_eq!(app.world().resource::<NativeShellModel>().screen, NativeShellScreen::Connecting);
        assert_eq!(app.world().resource::<QuestUiState>().detail_quest_index, None);
    }
}
