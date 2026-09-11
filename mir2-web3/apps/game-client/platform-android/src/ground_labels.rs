//! Android-only world labels for authoritative ground drops.
//!
//! This is a presentation adapter over the same validated `groundDrops` used
//! by the renderer and pickup UI. It never decides pickup eligibility or
//! mutates object state.

use bevy::{prelude::*, text::LineBreak, ui::FocusPolicy};
use mir2_client_bevy::{
    crystal_ui::typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX},
    native_shell::{NativeShellModel, NativeShellScreen},
};
use serde_json::Value;
use std::collections::HashSet;

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const ENTITY_LEFT_ORIGIN: f32 = 480.0;
const ENTITY_TOP_ORIGIN: f32 = 352.0;
const LABEL_LEFT_OFFSET: f32 = -16.0;
const LABEL_TOP_OFFSET: f32 = -18.0;
const LABEL_WIDTH: f32 = 80.0;
const OVERLAY_Z_INDEX: i32 = 850;
const MAX_MODEL_BYTES: usize = 2 * 1024 * 1024;
const MAX_WORLD_OBJECTS: usize = 8192;
const MAX_VISIBLE_LABELS: usize = 256;

#[derive(Component)]
pub(crate) struct GroundDropLabelRoot;

#[derive(Clone, Debug, PartialEq)]
struct GroundDropLabel {
    object_id: u32,
    text: String,
    color: [u8; 4],
    left: f32,
    top: f32,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct GroundDropLabelModel {
    center: Option<(i32, i32)>,
    labels: Vec<GroundDropLabel>,
    revision: u64,
}

impl GroundDropLabelModel {
    pub(crate) fn center(&self) -> Option<(i32, i32)> {
        self.center
    }

    pub(crate) fn replace(&mut self, projected: ProjectedGroundDropLabels) {
        if self.center == Some(projected.center) && self.labels == projected.labels {
            return;
        }
        self.center = Some(projected.center);
        self.labels = projected.labels;
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn reset(&mut self) {
        if self.center.is_none() && self.labels.is_empty() {
            return;
        }
        self.center = None;
        self.labels.clear();
        self.revision = self.revision.wrapping_add(1);
    }
}

#[derive(Debug)]
pub(crate) struct ProjectedGroundDropLabels {
    center: (i32, i32),
    labels: Vec<GroundDropLabel>,
}

pub(crate) fn project(
    models_json: &str,
    center_x: i32,
    center_y: i32,
) -> Option<ProjectedGroundDropLabels> {
    if models_json.len() > MAX_MODEL_BYTES {
        return None;
    }
    let models: Value = serde_json::from_str(models_json).ok()?;
    let entities = models.get("entities")?.as_array()?;
    let drops = match models.get("groundDrops") {
        None => &[][..],
        Some(value) => value.as_array()?.as_slice(),
    };
    if entities
        .len()
        .checked_add(drops.len())
        .is_none_or(|count| count > MAX_WORLD_OBJECTS)
    {
        return None;
    }

    let mut ids = HashSet::with_capacity(entities.len().saturating_add(drops.len()));
    for entity in entities {
        let object_id = object_id(entity.get("objectId")?)?;
        if !ids.insert(object_id)
            || coordinate(entity.get("x")?).is_none()
            || coordinate(entity.get("y")?).is_none()
        {
            return None;
        }
    }

    let mut labels = Vec::new();
    for drop in drops {
        let object_id = object_id(drop.get("objectId")?)?;
        let x = coordinate(drop.get("x")?)?;
        let y = coordinate(drop.get("y")?)?;
        let name = drop.get("name")?.as_str()?.trim();
        let quantity = drop
            .get("quantity")?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)?;
        let color = argb(
            drop.get("nameColourArgb")?
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())?,
        );
        if !ids.insert(object_id) || name.is_empty() || name.chars().count() > 128 {
            return None;
        }
        let dx = x.saturating_sub(center_x);
        let dy = y.saturating_sub(center_y);
        if dx.unsigned_abs() > 24 || dy.unsigned_abs() > 32 {
            continue;
        }
        labels.push(GroundDropLabel {
            object_id,
            text: if quantity == 1 {
                name.to_owned()
            } else {
                format!("{name} x{quantity}")
            },
            color,
            left: ENTITY_LEFT_ORIGIN + dx as f32 * CELL_WIDTH + LABEL_LEFT_OFFSET,
            top: ENTITY_TOP_ORIGIN + dy as f32 * CELL_HEIGHT + LABEL_TOP_OFFSET,
        });
    }
    labels.sort_by_key(|label| label.object_id);
    labels.truncate(MAX_VISIBLE_LABELS);
    Some(ProjectedGroundDropLabels {
        center: (center_x, center_y),
        labels,
    })
}

fn object_id(value: &Value) -> Option<u32> {
    let object_id = value
        .as_str()
        .and_then(|value| value.parse::<u32>().ok())
        .or_else(|| value.as_u64().and_then(|value| u32::try_from(value).ok()))?;
    (object_id != 0).then_some(object_id)
}

fn coordinate(value: &Value) -> Option<i32> {
    i32::try_from(value.as_i64()?).ok()
}

fn argb(value: i32) -> [u8; 4] {
    let bits = value as u32;
    [
        ((bits >> 16) & 0xff) as u8,
        ((bits >> 8) & 0xff) as u8,
        (bits & 0xff) as u8,
        ((bits >> 24) & 0xff) as u8,
    ]
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<GroundDropLabelModel>()
        .add_systems(Update, sync);
}

fn sync(
    mut commands: Commands,
    shell: Res<NativeShellModel>,
    labels: Res<GroundDropLabelModel>,
    player: Option<Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
    roots: Query<Entity, With<GroundDropLabelRoot>>,
    mut rendered: Local<Option<(u64, bool)>>,
) {
    let drop_view = player.as_deref().is_none_or(|state| {
        state.core.options.drop_view || state.local_keys.drops_visible(std::time::Instant::now())
    });
    let visible = shell.screen == NativeShellScreen::InGame && drop_view;
    if rendered.as_ref() == Some(&(labels.revision, visible)) {
        return;
    }
    *rendered = Some((labels.revision, visible));
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !visible || labels.center.is_none() || labels.labels.is_empty() {
        return;
    }

    commands
        .spawn((
            GroundDropLabelRoot,
            FocusPolicy::Pass,
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_WIDTH),
                height: px(STAGE_HEIGHT),
                ..default()
            },
            GlobalZIndex(OVERLAY_Z_INDEX),
        ))
        .with_children(|root| {
            for label in &labels.labels {
                for (offset, color) in crystal_outline_offsets()
                    .into_iter()
                    .map(|offset| (offset, Color::BLACK))
                    .chain(std::iter::once((
                        Vec2::ONE,
                        Color::srgba_u8(
                            label.color[0],
                            label.color[1],
                            label.color[2],
                            label.color[3],
                        ),
                    )))
                {
                    root.spawn((
                        FocusPolicy::Pass,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(label.left + offset.x),
                            top: px(label.top + offset.y),
                            width: px(LABEL_WIDTH),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                    ))
                    .with_children(|line| {
                        line.spawn((
                            FocusPolicy::Pass,
                            Node {
                                flex_shrink: 0.0,
                                ..default()
                            },
                            Text::new(label.text.clone()),
                            crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX),
                            TextColor(color),
                            TextLayout::new(Justify::Center, LineBreak::NoWrap),
                        ));
                    });
                }
            }
        });
}

fn crystal_outline_offsets() -> [Vec2; 4] {
    [
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(2.0, 1.0),
        Vec2::new(1.0, 2.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_labels_keep_identity_quantity_color_and_crystal_anchor() {
        let models = serde_json::json!({
            "entities":[{"objectId":"7","kind":"selfPlayer","x":10,"y":20}],
            "groundDrops":[
                {"objectId":"44","name":"Potion","nameColourArgb":-15584170,
                 "x":11,"y":21,"quantity":2},
                {"objectId":"43","name":"Gold","nameColourArgb":-256,
                 "x":10,"y":20,"quantity":250}
            ]
        });
        let projected = project(&models.to_string(), 10, 20).expect("valid labels");
        assert_eq!(projected.labels.len(), 2);
        assert_eq!(projected.labels[0].object_id, 43);
        assert_eq!(projected.labels[0].text, "Gold x250");
        assert_eq!(projected.labels[0].color, [0xff, 0xff, 0x00, 0xff]);
        assert_eq!(
            (projected.labels[0].left, projected.labels[0].top),
            (464.0, 334.0)
        );
        assert_eq!(projected.labels[1].text, "Potion x2");
        assert_eq!(projected.labels[1].color, [0x12, 0x34, 0x56, 0xff]);
        assert_eq!(
            (projected.labels[1].left, projected.labels[1].top),
            (512.0, 366.0)
        );
    }

    #[test]
    fn malformed_or_cross_kind_duplicate_identity_is_rejected_atomically() {
        for models in [
            serde_json::json!({"entities":[],"groundDrops":[
                {"objectId":"1","name":"Bad","nameColourArgb":-1,"x":0,"y":0,"quantity":0}
            ]}),
            serde_json::json!({"entities":[{"objectId":"1","x":0,"y":0}],"groundDrops":[
                {"objectId":"1","name":"Bad","nameColourArgb":-1,"x":0,"y":0,"quantity":1}
            ]}),
        ] {
            assert!(project(&models.to_string(), 0, 0).is_none());
        }
    }

    #[test]
    fn replacement_and_reset_advance_only_when_presentation_changes() {
        let models = serde_json::json!({
            "entities":[{"objectId":"7","x":10,"y":20}],
            "groundDrops":[{"objectId":"44","name":"Potion","nameColourArgb":-1,
                "x":11,"y":21,"quantity":1}]
        });
        let mut model = GroundDropLabelModel::default();
        model.replace(project(&models.to_string(), 10, 20).unwrap());
        assert_eq!(model.revision, 1);
        model.replace(project(&models.to_string(), 10, 20).unwrap());
        assert_eq!(model.revision, 1);
        model.reset();
        assert_eq!(model.revision, 2);
        model.reset();
        assert_eq!(model.revision, 2);
    }

    #[test]
    fn visible_label_tree_is_pass_through_and_honors_shared_drop_view() {
        use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;

        let models = serde_json::json!({
            "entities":[{"objectId":"7","x":10,"y":20}],
            "groundDrops":[{"objectId":"44","name":"Potion","nameColourArgb":-1,
                "x":11,"y":21,"quantity":1}]
        });
        let mut label_model = GroundDropLabelModel::default();
        label_model.replace(project(&models.to_string(), 10, 20).unwrap());
        let mut player = NativePlayerUiState::default();
        player.core.options.drop_view = true;
        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(player)
        .insert_resource(label_model)
        .add_systems(Update, sync);
        app.update();

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<GroundDropLabelRoot>>()
                .iter(app.world())
                .count(),
            1
        );
        assert!(app
            .world_mut()
            .query::<&FocusPolicy>()
            .iter(app.world())
            .all(|policy| *policy == FocusPolicy::Pass));

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .options
            .drop_view = false;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<GroundDropLabelRoot>>()
                .iter(app.world())
                .count(),
            0
        );
    }
}
