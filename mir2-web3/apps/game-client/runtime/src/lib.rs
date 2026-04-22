use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::window::WindowResolution;
use js_sys::Function;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

const TILE_SIZE: f32 = 32.0;
const FLOOR_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);

thread_local! {
    static STATUS_SINK: RefCell<Option<Function>> = const { RefCell::new(None) };
    static PENDING_WORLD_STATE: RefCell<Option<WorldSnapshot>> = const { RefCell::new(None) };
}

#[derive(Resource, Default, Clone)]
struct RuntimeWorldState {
    snapshot: Option<WorldSnapshot>,
}

#[derive(Resource, Default)]
struct SceneRegistry {
    entities: HashMap<String, SceneEntityHandles>,
    map: MapSceneCache,
}

#[derive(Default)]
struct MapSceneCache {
    blueprint: Option<MapSceneBlueprint>,
    spawned: Vec<Entity>,
}

#[derive(Clone, Copy)]
struct SceneEntityHandles {
    root: Entity,
    shadow: Entity,
    body: Entity,
    crest: Entity,
    facing: Entity,
    selection: Entity,
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct MirObject;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorldSnapshot {
    #[serde(default)]
    map_title: Option<String>,
    #[serde(default)]
    player_object_id: Option<String>,
    #[serde(default)]
    selected_object_id: Option<String>,
    #[serde(default)]
    scene_view: Option<SceneView>,
    #[serde(default)]
    terrain_patches: Vec<TerrainPatch>,
    #[serde(default)]
    decor_objects: Vec<DecorObject>,
    #[serde(default)]
    entities: Vec<WorldEntity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneView {
    center: GridPoint,
    width: u16,
    height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridPoint {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerrainPatch {
    x: i32,
    y: i32,
    width: u16,
    height: u16,
    kind: TerrainKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TerrainKind {
    Grass,
    Dirt,
    Road,
    Water,
    Stone,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecorObject {
    id: String,
    x: i32,
    y: i32,
    kind: DecorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DecorKind {
    Lantern,
    Banner,
    Tree,
    Rock,
    Campfire,
    Stump,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorldEntity {
    object_id: String,
    kind: EntityKind,
    name: String,
    x: i32,
    y: i32,
    direction: Option<String>,
    level: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum EntityKind {
    Player,
    SelfPlayer,
    Monster,
    Npc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MapSceneBlueprint {
    map_title: Option<String>,
    scene_view: Option<SceneView>,
    terrain_patches: Vec<TerrainPatch>,
    decor_objects: Vec<DecorObject>,
}

#[wasm_bindgen(js_name = setMir2StatusSink)]
pub fn set_mir2_status_sink(callback: Function) {
    STATUS_SINK.with(|sink| {
        sink.borrow_mut().replace(callback);
    });
}

#[wasm_bindgen(js_name = clearMir2StatusSink)]
pub fn clear_mir2_status_sink() {
    STATUS_SINK.with(|sink| {
        sink.borrow_mut().take();
    });
}

#[wasm_bindgen(js_name = setMir2WorldState)]
pub fn set_mir2_world_state(snapshot_json: String) {
    match serde_json::from_str::<WorldSnapshot>(&snapshot_json) {
        Ok(snapshot) => {
            PENDING_WORLD_STATE.with(|pending| {
                *pending.borrow_mut() = Some(snapshot);
            });
        }
        Err(error) => publish_status("decode-error", &error.to_string()),
    }
}

#[wasm_bindgen(js_name = bootMir2Runtime)]
pub fn boot_mir2_runtime() {
    console_error_panic_hook::set_once();

    publish_status("runtime-entered", "Bevy runtime entry reached");

    let mut app = App::new();
    app.insert_resource(ClearColor(FLOOR_COLOR))
        .insert_resource(RuntimeWorldState::default())
        .insert_resource(SceneRegistry::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#mir2-web3-canvas".to_owned()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                resolution: WindowResolution::new(1280, 720),
                title: "mir2-web3".to_owned(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                ingest_pending_world_state,
                sync_map_scene,
                sync_entities,
                follow_player,
            ),
        );

    publish_status("running", "Handing off to Bevy app loop");
    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));
    publish_status("scene-ready", "Camera ready");
}

fn ingest_pending_world_state(mut state: ResMut<RuntimeWorldState>) {
    PENDING_WORLD_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            state.snapshot = Some(snapshot);
        }
    });
}

fn sync_map_scene(
    mut commands: Commands,
    state: Res<RuntimeWorldState>,
    mut registry: ResMut<SceneRegistry>,
) {
    let Some(snapshot) = &state.snapshot else {
        return;
    };

    let blueprint = MapSceneBlueprint::from_snapshot(snapshot);
    if registry.map.blueprint.as_ref() == Some(&blueprint) {
        return;
    }

    for entity in registry.map.spawned.drain(..) {
        commands.entity(entity).despawn();
    }

    registry.map.spawned = spawn_map_scene(&mut commands, &blueprint);
    let title = blueprint
        .map_title
        .clone()
        .unwrap_or_else(|| "unknown".to_owned());
    registry.map.blueprint = Some(blueprint);
    publish_status("scene-synced", &format!("Applied scene blueprint for {title}"));
}

fn sync_entities(
    mut commands: Commands,
    state: Res<RuntimeWorldState>,
    mut registry: ResMut<SceneRegistry>,
    mut transform_query: Query<&mut Transform>,
    mut sprite_query: Query<&mut Sprite>,
) {
    let Some(snapshot) = &state.snapshot else {
        return;
    };

    let mut alive = HashSet::new();

    for entity_data in &snapshot.entities {
        alive.insert(entity_data.object_id.clone());
        let position = tile_to_world(entity_data.x, entity_data.y);
        let is_selected = snapshot
            .selected_object_id
            .as_ref()
            .is_some_and(|selected_id| selected_id == &entity_data.object_id);

        if let Some(handles) = registry.entities.get(&entity_data.object_id).copied() {
            if let Ok(mut transform) = transform_query.get_mut(handles.root) {
                transform.translation = position;
            }
            update_entity_visuals(
                handles,
                entity_data,
                is_selected,
                &mut transform_query,
                &mut sprite_query,
            );
            continue;
        }

        let mut shadow = None;
        let mut body = None;
        let mut crest = None;
        let mut facing = None;
        let mut selection = None;
        let entity = commands
            .spawn((MirObject, Transform::from_translation(position)))
            .with_children(|parent| {
                selection = Some(
                    parent
                        .spawn((
                            Sprite::from_color(Color::NONE, Vec2::new(36.0, 36.0)),
                            Transform::from_xyz(0.0, -8.0, 0.0),
                        ))
                        .id(),
                );
                shadow = Some(
                    parent
                        .spawn((
                            Sprite::from_color(
                                Color::srgba(0.02, 0.02, 0.02, 0.22),
                                Vec2::new(26.0, 10.0),
                            ),
                            Transform::from_xyz(0.0, -10.0, 0.5),
                        ))
                        .id(),
                );
                body = Some(
                    parent
                        .spawn((
                            Sprite::from_color(entity_data.kind.color(), entity_data.body_size()),
                            Transform::from_xyz(0.0, 6.0, 1.0),
                        ))
                        .id(),
                );
                crest = Some(
                    parent
                        .spawn((
                            Sprite::from_color(
                                entity_data.kind.accent_color(&entity_data.name),
                                entity_data.crest_size(entity_data.body_size()),
                            ),
                            Transform::from_xyz(0.0, 16.0, 2.0),
                        ))
                        .id(),
                );
                facing = Some(
                    parent
                        .spawn((
                            Sprite::from_color(
                                entity_data.kind.facing_color(),
                                entity_data.kind.facing_size(),
                            ),
                            Transform::from_xyz(0.0, 0.0, 3.0),
                        ))
                        .id(),
                );
            })
            .id();

        let handles = SceneEntityHandles {
            root: entity,
            shadow: shadow.expect("shadow child"),
            body: body.expect("body child"),
            crest: crest.expect("crest child"),
            facing: facing.expect("facing child"),
            selection: selection.expect("selection child"),
        };

        update_entity_visuals(
            handles,
            entity_data,
            is_selected,
            &mut transform_query,
            &mut sprite_query,
        );

        registry
            .entities
            .insert(entity_data.object_id.clone(), handles);
    }

    let stale_ids: Vec<String> = registry
        .entities
        .keys()
        .filter(|object_id| !alive.contains(*object_id))
        .cloned()
        .collect();

    for object_id in stale_ids {
        if let Some(handles) = registry.entities.remove(&object_id) {
            commands.entity(handles.root).despawn();
        }
    }
}

fn follow_player(
    state: Res<RuntimeWorldState>,
    mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<MirObject>)>,
) {
    let Some(snapshot) = &state.snapshot else {
        return;
    };

    let focus = snapshot
        .player_object_id
        .as_ref()
        .and_then(|player_object_id| {
            snapshot
                .entities
                .iter()
                .find(|entity| &entity.object_id == player_object_id)
                .map(|entity| GridPoint {
                    x: entity.x,
                    y: entity.y,
                })
        })
        .or_else(|| snapshot.scene_view.as_ref().map(|scene| scene.center));

    let Some(focus) = focus else {
        return;
    };

    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    camera_transform.translation.x = focus.x as f32 * TILE_SIZE;
    camera_transform.translation.y = -(focus.y as f32) * TILE_SIZE;
    camera_transform.translation.z = 1000.0;
}

fn tile_to_world(x: i32, y: i32) -> Vec3 {
    Vec3::new(x as f32 * TILE_SIZE, -(y as f32) * TILE_SIZE, 1.0)
}

fn spawn_map_scene(commands: &mut Commands, blueprint: &MapSceneBlueprint) -> Vec<Entity> {
    let Some(scene_view) = &blueprint.scene_view else {
        return Vec::new();
    };

    let mut spawned = Vec::new();
    let start_x = scene_view.center.x - i32::from(scene_view.width / 2);
    let start_y = scene_view.center.y - i32::from(scene_view.height / 2);

    for offset_y in 0..i32::from(scene_view.height) {
        for offset_x in 0..i32::from(scene_view.width) {
            let x = start_x + offset_x;
            let y = start_y + offset_y;
            let terrain = terrain_kind_at(x, y, &blueprint.terrain_patches);
            let variation = tile_variation(x, y);
            let mut translation = tile_to_world(x, y);
            translation.z = -0.75;

            let root = commands
                .spawn(Transform::from_translation(translation))
                .with_children(|parent| {
                    parent.spawn((
                        Sprite::from_color(terrain.base_color(variation), Vec2::splat(TILE_SIZE - 1.0)),
                        Transform::from_xyz(0.0, 0.0, 0.0),
                    ));
                    parent.spawn((
                        Sprite::from_color(
                            terrain.accent_color(variation),
                            terrain.accent_size(variation),
                        ),
                        Transform::from_xyz(
                            terrain.accent_offset(variation).x,
                            terrain.accent_offset(variation).y,
                            0.05,
                        ),
                    ));
                    parent.spawn((
                        Sprite::from_color(
                            Color::srgba(0.02, 0.01, 0.01, 0.05),
                            Vec2::new(TILE_SIZE - 2.0, 1.5),
                        ),
                        Transform::from_xyz(0.0, -(TILE_SIZE * 0.5) + 2.0, 0.08),
                    ));
                })
                .id();
            spawned.push(root);
        }
    }

    for decor in &blueprint.decor_objects {
        let mut translation = tile_to_world(decor.x, decor.y);
        translation.z = 0.3;

        let entity = commands
            .spawn(Transform::from_translation(translation))
            .with_children(|parent| {
                parent.spawn((
                    Sprite::from_color(Color::srgba(0.02, 0.02, 0.02, 0.10), Vec2::new(18.0, 8.0)),
                    Transform::from_xyz(0.0, -10.0, 0.0),
                ));

                match decor.kind {
                    DecorKind::Lantern => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.33, 0.24, 0.16), Vec2::new(4.0, 26.0)),
                            Transform::from_xyz(0.0, 8.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.95, 0.71, 0.28), Vec2::new(10.0, 10.0)),
                            Transform::from_xyz(0.0, 18.0, 0.1),
                        ));
                        parent.spawn((
                            Sprite::from_color(Color::srgba(1.0, 0.82, 0.34, 0.24), Vec2::new(18.0, 18.0)),
                            Transform::from_xyz(0.0, 18.0, 0.08),
                        ));
                    }
                    DecorKind::Banner => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.28, 0.20, 0.15), Vec2::new(4.0, 30.0)),
                            Transform::from_xyz(0.0, 8.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.70, 0.21, 0.16), Vec2::new(15.0, 18.0)),
                            Transform::from_xyz(8.0, 16.0, 0.1),
                        ));
                    }
                    DecorKind::Tree => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.31, 0.20, 0.12), Vec2::new(8.0, 18.0)),
                            Transform::from_xyz(0.0, 4.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.18, 0.40, 0.15), Vec2::new(28.0, 22.0)),
                            Transform::from_xyz(0.0, 20.0, 0.1),
                        ));
                    }
                    DecorKind::Rock => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.46, 0.43, 0.38), Vec2::new(22.0, 14.0)),
                            Transform::from_xyz(0.0, 2.0, 0.08),
                        ));
                    }
                    DecorKind::Campfire => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.33, 0.21, 0.12), Vec2::new(18.0, 6.0)),
                            Transform::from_xyz(0.0, -2.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.96, 0.57, 0.19), Vec2::new(10.0, 14.0)),
                            Transform::from_xyz(0.0, 8.0, 0.1),
                        ));
                        parent.spawn((
                            Sprite::from_color(Color::srgba(1.0, 0.67, 0.24, 0.22), Vec2::new(26.0, 22.0)),
                            Transform::from_xyz(0.0, 6.0, 0.08),
                        ));
                    }
                    DecorKind::Stump => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.42, 0.29, 0.17), Vec2::new(14.0, 10.0)),
                            Transform::from_xyz(0.0, 0.0, 0.08),
                        ));
                    }
                }
            })
            .id();
        spawned.push(entity);
    }

    spawned
}

fn update_entity_visuals(
    handles: SceneEntityHandles,
    entity_data: &WorldEntity,
    is_selected: bool,
    transform_query: &mut Query<&mut Transform>,
    sprite_query: &mut Query<&mut Sprite>,
) {
    let body_size = entity_data.body_size();
    let facing_offset = direction_to_world_offset(entity_data.direction.as_deref(), body_size);

    if let Ok(mut sprite) = sprite_query.get_mut(handles.body) {
        sprite.color = entity_data.kind.color();
        sprite.custom_size = Some(body_size);
    }

    if let Ok(mut transform) = transform_query.get_mut(handles.body) {
        transform.translation = Vec3::new(0.0, body_size.y * 0.2 - 2.0, 1.0);
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.shadow) {
        sprite.custom_size = Some(Vec2::new(body_size.x + 10.0, 10.0));
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.selection) {
        sprite.color = if is_selected {
            entity_data.kind.selection_color()
        } else {
            Color::NONE
        };
        sprite.custom_size = Some(body_size + Vec2::new(18.0, 12.0));
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.crest) {
        sprite.color = Color::NONE;
        sprite.custom_size = Some(entity_data.crest_size(body_size));
    }

    if let Ok(mut transform) = transform_query.get_mut(handles.crest) {
        transform.translation = Vec3::new(0.0, body_size.y * 0.45, 2.0);
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.facing) {
        sprite.color = Color::NONE;
        sprite.custom_size = Some(entity_data.kind.facing_size());
    }

    if let Ok(mut transform) = transform_query.get_mut(handles.facing) {
        transform.translation = Vec3::new(facing_offset.x, facing_offset.y + 4.0, 3.0);
    }
}

fn direction_to_world_offset(direction: Option<&str>, body_size: Vec2) -> Vec2 {
    let vector = match direction.unwrap_or("Down") {
        "Up" => Vec2::new(0.0, 1.0),
        "UpRight" => Vec2::new(1.0, 1.0).normalize(),
        "Right" => Vec2::new(1.0, 0.0),
        "DownRight" => Vec2::new(1.0, -1.0).normalize(),
        "Down" => Vec2::new(0.0, -1.0),
        "DownLeft" => Vec2::new(-1.0, -1.0).normalize(),
        "Left" => Vec2::new(-1.0, 0.0),
        "UpLeft" => Vec2::new(-1.0, 1.0).normalize(),
        _ => Vec2::new(0.0, -1.0),
    };

    Vec2::new(
        vector.x * (body_size.x * 0.5 + 7.0),
        vector.y * (body_size.y * 0.5 + 7.0),
    )
}

impl EntityKind {
    fn color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgb(0.74, 0.58, 0.28),
            Self::Player => Color::srgb(0.60, 0.55, 0.42),
            Self::Monster => Color::srgb(0.55, 0.27, 0.18),
            Self::Npc => Color::srgb(0.48, 0.57, 0.33),
        }
    }

    fn size(self) -> Vec2 {
        match self {
            Self::SelfPlayer | Self::Player => Vec2::new(24.0, 32.0),
            Self::Monster => Vec2::new(28.0, 28.0),
            Self::Npc => Vec2::new(20.0, 30.0),
        }
    }

    fn selection_color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgba(0.96, 0.75, 0.26, 0.22),
            Self::Player => Color::srgba(0.34, 0.67, 0.91, 0.18),
            Self::Monster => Color::srgba(0.91, 0.35, 0.24, 0.18),
            Self::Npc => Color::srgba(0.44, 0.78, 0.47, 0.18),
        }
    }

    fn facing_color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgb(1.0, 0.92, 0.74),
            Self::Player => Color::srgb(0.84, 0.93, 1.0),
            Self::Monster => Color::srgb(1.0, 0.82, 0.74),
            Self::Npc => Color::srgb(0.89, 1.0, 0.87),
        }
    }

    fn facing_size(self) -> Vec2 {
        match self {
            Self::SelfPlayer | Self::Player => Vec2::new(8.0, 8.0),
            Self::Monster => Vec2::new(10.0, 10.0),
            Self::Npc => Vec2::new(7.0, 7.0),
        }
    }

    fn accent_color(self, name: &str) -> Color {
        let variant = name_seed(name) % 3;
        match (self, variant) {
            (Self::SelfPlayer, 0) => Color::srgb(1.0, 0.92, 0.74),
            (Self::SelfPlayer, 1) => Color::srgb(0.95, 0.83, 0.58),
            (Self::SelfPlayer, _) => Color::srgb(0.98, 0.87, 0.70),
            (Self::Player, 0) => Color::srgb(0.78, 0.92, 1.0),
            (Self::Player, 1) => Color::srgb(0.67, 0.84, 0.98),
            (Self::Player, _) => Color::srgb(0.58, 0.77, 0.94),
            (Self::Monster, 0) => Color::srgb(1.0, 0.70, 0.46),
            (Self::Monster, 1) => Color::srgb(0.94, 0.59, 0.34),
            (Self::Monster, _) => Color::srgb(0.91, 0.48, 0.29),
            (Self::Npc, 0) => Color::srgb(0.87, 1.0, 0.76),
            (Self::Npc, 1) => Color::srgb(0.72, 0.95, 0.66),
            (Self::Npc, _) => Color::srgb(0.63, 0.88, 0.58),
        }
    }
}

impl WorldEntity {
    fn body_size(&self) -> Vec2 {
        let mut size = self.kind.size();
        let level_bonus = f32::from(self.level.unwrap_or(1).min(30)) * 0.12;
        size.x += level_bonus;
        size.y += level_bonus * 1.8;
        size
    }

    fn crest_size(&self, body_size: Vec2) -> Vec2 {
        match self.kind {
            EntityKind::SelfPlayer | EntityKind::Player => Vec2::new(
                body_size.x * 0.45,
                8.0 + f32::from(self.level.unwrap_or(1).min(20)) * 0.15,
            ),
            EntityKind::Monster => Vec2::new(body_size.x * 0.6, body_size.y * 0.18),
            EntityKind::Npc => Vec2::new(body_size.x * 0.4, body_size.y * 0.18),
        }
    }
}

impl MapSceneBlueprint {
    fn from_snapshot(snapshot: &WorldSnapshot) -> Self {
        Self {
            map_title: snapshot.map_title.clone(),
            scene_view: snapshot.scene_view.clone(),
            terrain_patches: snapshot.terrain_patches.clone(),
            decor_objects: snapshot.decor_objects.clone(),
        }
    }
}

impl TerrainKind {
    fn base_color(self, variation: u32) -> Color {
        match self {
            Self::Grass => match variation % 3 {
                0 => Color::srgba(0.16, 0.24, 0.12, 0.28),
                1 => Color::srgba(0.18, 0.27, 0.14, 0.28),
                _ => Color::srgba(0.14, 0.22, 0.11, 0.28),
            },
            Self::Dirt => match variation % 3 {
                0 => Color::srgba(0.34, 0.22, 0.13, 0.30),
                1 => Color::srgba(0.30, 0.20, 0.11, 0.30),
                _ => Color::srgba(0.36, 0.24, 0.15, 0.30),
            },
            Self::Road => match variation % 3 {
                0 => Color::srgba(0.45, 0.35, 0.23, 0.30),
                1 => Color::srgba(0.40, 0.31, 0.20, 0.30),
                _ => Color::srgba(0.48, 0.38, 0.26, 0.30),
            },
            Self::Water => match variation % 3 {
                0 => Color::srgba(0.09, 0.25, 0.31, 0.26),
                1 => Color::srgba(0.11, 0.30, 0.37, 0.26),
                _ => Color::srgba(0.08, 0.22, 0.28, 0.26),
            },
            Self::Stone => match variation % 3 {
                0 => Color::srgba(0.42, 0.39, 0.34, 0.28),
                1 => Color::srgba(0.38, 0.35, 0.30, 0.28),
                _ => Color::srgba(0.46, 0.43, 0.37, 0.28),
            },
        }
    }

    fn accent_color(self, variation: u32) -> Color {
        match self {
            Self::Grass => match variation % 3 {
                0 => Color::srgba(0.26, 0.40, 0.20, 0.18),
                1 => Color::srgba(0.22, 0.34, 0.18, 0.18),
                _ => Color::srgba(0.29, 0.43, 0.24, 0.18),
            },
            Self::Dirt => match variation % 3 {
                0 => Color::srgba(0.46, 0.30, 0.18, 0.20),
                1 => Color::srgba(0.40, 0.27, 0.15, 0.20),
                _ => Color::srgba(0.52, 0.34, 0.20, 0.20),
            },
            Self::Road => match variation % 3 {
                0 => Color::srgba(0.56, 0.46, 0.31, 0.22),
                1 => Color::srgba(0.50, 0.40, 0.28, 0.22),
                _ => Color::srgba(0.60, 0.49, 0.34, 0.22),
            },
            Self::Water => match variation % 3 {
                0 => Color::srgba(0.20, 0.48, 0.58, 0.20),
                1 => Color::srgba(0.24, 0.56, 0.66, 0.20),
                _ => Color::srgba(0.16, 0.42, 0.54, 0.20),
            },
            Self::Stone => match variation % 3 {
                0 => Color::srgba(0.56, 0.54, 0.48, 0.18),
                1 => Color::srgba(0.50, 0.48, 0.43, 0.18),
                _ => Color::srgba(0.62, 0.59, 0.53, 0.18),
            },
        }
    }

    fn accent_size(self, variation: u32) -> Vec2 {
        match self {
            Self::Grass => {
                if variation % 2 == 0 {
                    Vec2::new(10.0, 12.0)
                } else {
                    Vec2::new(14.0, 8.0)
                }
            }
            Self::Dirt => Vec2::new(16.0, 10.0),
            Self::Road => Vec2::new(18.0, 6.0),
            Self::Water => Vec2::new(20.0, 10.0),
            Self::Stone => Vec2::new(16.0, 12.0),
        }
    }

    fn accent_offset(self, variation: u32) -> Vec2 {
        match self {
            Self::Grass => match variation % 3 {
                0 => Vec2::new(-8.0, 7.0),
                1 => Vec2::new(7.0, -5.0),
                _ => Vec2::new(3.0, 8.0),
            },
            Self::Dirt => Vec2::new(-4.0, 5.0),
            Self::Road => Vec2::new(0.0, -7.0),
            Self::Water => Vec2::new(5.0, 6.0),
            Self::Stone => Vec2::new(-5.0, -3.0),
        }
    }
}

fn terrain_kind_at(x: i32, y: i32, patches: &[TerrainPatch]) -> TerrainKind {
    patches
        .iter()
        .rev()
        .find(|patch| patch_contains(patch, x, y))
        .map(|patch| patch.kind)
        .unwrap_or(TerrainKind::Grass)
}

fn patch_contains(patch: &TerrainPatch, x: i32, y: i32) -> bool {
    x >= patch.x
        && x < patch.x + i32::from(patch.width)
        && y >= patch.y
        && y < patch.y + i32::from(patch.height)
}

fn tile_variation(x: i32, y: i32) -> u32 {
    let ux = u32::from_ne_bytes(x.to_ne_bytes());
    let uy = u32::from_ne_bytes(y.to_ne_bytes());
    (ux.wrapping_mul(31)).wrapping_add(uy.wrapping_mul(17))
}

fn name_seed(name: &str) -> u8 {
    name.bytes().fold(0u8, |acc, byte| acc.wrapping_add(byte))
}

fn publish_status(phase: &str, message: &str) {
    STATUS_SINK.with(|sink| {
        let callback_ref = sink.borrow();
        let Some(callback) = callback_ref.as_ref() else {
            return;
        };

        let payload = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("phase"),
            &JsValue::from_str(phase),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("message"),
            &JsValue::from_str(message),
        );
        let _ = callback.call1(&JsValue::NULL, &payload.into());
    });
}
