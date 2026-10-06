//! Keep shared overflow bounds aligned with Android's magnified focus windows.
//!
//! Bevy 0.19's clip updater applies only UiGlobalTransform.translation. Android
//! focus windows also scale their descendants, so the default clip truncates
//! mail text and frame sprites after the keyboard closes. Run after PostLayout
//! using the same hierarchy/overflow semantics, with affine-transformed bounds.
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::ui::experimental::{UiChildren, UiRootNodes};
use bevy::ui::{CalculatedClip, OverrideClip};

fn scaled_interval(coefficient: f32, minimum: f32, maximum: f32) -> (f32, f32) {
    // A visible overflow axis has infinite bounds. Avoid 0 * infinity, which
    // would produce NaN even for the ordinary axis-aligned scale transform.
    if coefficient == 0.0 {
        (0.0, 0.0)
    } else if coefficient > 0.0 {
        (coefficient * minimum, coefficient * maximum)
    } else {
        (coefficient * maximum, coefficient * minimum)
    }
}

fn transformed_clip(local: Rect, transform: Affine2) -> Rect {
    let x_from_x = scaled_interval(transform.matrix2.x_axis.x, local.min.x, local.max.x);
    let x_from_y = scaled_interval(transform.matrix2.y_axis.x, local.min.y, local.max.y);
    let y_from_x = scaled_interval(transform.matrix2.x_axis.y, local.min.x, local.max.x);
    let y_from_y = scaled_interval(transform.matrix2.y_axis.y, local.min.y, local.max.y);
    Rect {
        min: Vec2::new(x_from_x.0 + x_from_y.0, y_from_x.0 + y_from_y.0) + transform.translation,
        max: Vec2::new(x_from_x.1 + x_from_y.1, y_from_x.1 + y_from_y.1) + transform.translation,
    }
}

pub(crate) fn update_android_clipping(
    mut commands: Commands,
    roots: UiRootNodes,
    children: UiChildren,
    mut nodes: Query<(
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&mut CalculatedClip>,
        Has<OverrideClip>,
    )>,
) {
    let mut pending: Vec<(Entity, Option<Rect>)> =
        roots.iter().map(|entity| (entity, None)).collect();
    while let Some((entity, mut inherited)) = pending.pop() {
        let Ok((node, computed, transform, previous, override_clip)) = nodes.get_mut(entity) else {
            continue;
        };
        if override_clip {
            inherited = None;
        }
        if node.display == Display::None {
            inherited = Some(Rect::default());
        }
        match (previous, inherited) {
            (Some(mut previous), Some(clip)) => {
                if previous.clip != clip {
                    previous.clip = clip;
                }
            }
            (Some(_), None) => {
                commands.entity(entity).remove::<CalculatedClip>();
            }
            (None, Some(clip)) => {
                commands.entity(entity).try_insert(CalculatedClip { clip });
            }
            (None, None) => {}
        }
        let descendants_clip = if node.overflow.is_visible() {
            inherited
        } else {
            let clip = transformed_clip(
                computed.resolve_clip_rect(node.overflow, node.overflow_clip_margin),
                Affine2::from(transform),
            );
            Some(inherited.map_or(clip, |parent| parent.intersect(clip)))
        };
        pending.extend(
            children
                .iter_ui_children(entity)
                .map(|child| (child, descendants_clip)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui::{Overflow, OverflowAxis};

    #[test]
    fn clip_tracks_scaled_mail_window_and_translation() {
        assert_eq!(
            transformed_clip(
                Rect::from_corners(Vec2::new(-100.0, -80.0), Vec2::new(100.0, 80.0)),
                Affine2::from_scale_angle_translation(
                    Vec2::splat(2.5),
                    0.0,
                    Vec2::new(500.0, 400.0),
                ),
            ),
            Rect::from_corners(Vec2::new(250.0, 200.0), Vec2::new(750.0, 600.0)),
        );
    }

    #[test]
    fn visible_axis_remains_infinite_without_nan() {
        let clip = transformed_clip(
            Rect {
                min: Vec2::new(f32::NEG_INFINITY, -50.0),
                max: Vec2::new(f32::INFINITY, 50.0),
            },
            Affine2::from_scale_angle_translation(Vec2::new(3.0, 2.0), 0.0, Vec2::new(70.0, 100.0)),
        );
        assert_eq!(clip.min, Vec2::new(f32::NEG_INFINITY, 0.0));
        assert_eq!(clip.max, Vec2::new(f32::INFINITY, 200.0));
    }

    #[test]
    fn mirrored_transform_keeps_minimum_before_maximum() {
        assert_eq!(
            transformed_clip(
                Rect::from_corners(Vec2::ZERO, Vec2::new(10.0, 20.0)),
                Affine2::from_scale(Vec2::new(-2.0, 3.0)),
            ),
            Rect::from_corners(Vec2::new(-20.0, 0.0), Vec2::new(0.0, 60.0)),
        );
    }

    fn spawn_node(app: &mut App, node: Node, size: Vec2, transform: Affine2) -> Entity {
        app.world_mut()
            .spawn((
                node,
                ComputedNode { size, ..default() },
                UiGlobalTransform::from(transform),
            ))
            .id()
    }

    #[test]
    fn scaled_body_clip_intersects_parent_and_propagates_through_visible_nodes() {
        let mut app = App::new();
        app.add_systems(Update, update_android_clipping);
        let window = spawn_node(
            &mut app,
            Node {
                overflow: Overflow::clip(),
                ..default()
            },
            Vec2::splat(100.0),
            Affine2::from_scale_angle_translation(Vec2::splat(2.0), 0.0, Vec2::splat(200.0)),
        );
        let body = spawn_node(
            &mut app,
            Node {
                overflow: Overflow::clip(),
                ..default()
            },
            Vec2::new(80.0, 60.0),
            Affine2::from_scale_angle_translation(Vec2::splat(2.0), 0.0, Vec2::new(280.0, 240.0)),
        );
        app.world_mut().entity_mut(body).insert(ChildOf(window));
        let text = spawn_node(&mut app, Node::default(), Vec2::ONE, Affine2::IDENTITY);
        app.world_mut().entity_mut(text).insert(ChildOf(body));
        app.update();
        assert_eq!(
            app.world().get::<CalculatedClip>(body).unwrap().clip,
            Rect::from_corners(Vec2::splat(100.0), Vec2::splat(300.0)),
        );
        assert_eq!(
            app.world().get::<CalculatedClip>(text).unwrap().clip,
            Rect::from_corners(Vec2::new(200.0, 180.0), Vec2::splat(300.0)),
        );
    }

    #[test]
    fn display_none_override_and_removing_overflow_match_shared_semantics() {
        let mut app = App::new();
        app.add_systems(Update, update_android_clipping);
        let parent = spawn_node(
            &mut app,
            Node {
                display: Display::None,
                ..default()
            },
            Vec2::splat(100.0),
            Affine2::IDENTITY,
        );
        let child = spawn_node(&mut app, Node::default(), Vec2::ONE, Affine2::IDENTITY);
        app.world_mut().entity_mut(child).insert(ChildOf(parent));
        app.update();
        assert_eq!(
            app.world().get::<CalculatedClip>(child).unwrap().clip,
            Rect::default()
        );
        app.world_mut().entity_mut(child).insert(OverrideClip);
        app.update();
        assert!(app.world().get::<CalculatedClip>(child).is_none());
        app.world_mut()
            .entity_mut(parent)
            .get_mut::<Node>()
            .unwrap()
            .display = Display::Flex;
        app.world_mut().entity_mut(child).remove::<OverrideClip>();
        app.update();
        assert!(app.world().get::<CalculatedClip>(child).is_none());
    }

    #[test]
    fn partial_overflow_uses_transformed_bounds() {
        let computed = ComputedNode {
            size: Vec2::new(100.0, 50.0),
            ..default()
        };
        let local = computed.resolve_clip_rect(
            Overflow {
                x: OverflowAxis::Visible,
                y: OverflowAxis::Clip,
            },
            Default::default(),
        );
        let clip = transformed_clip(local, Affine2::from_scale(Vec2::splat(2.0)));
        assert_eq!(clip.min, Vec2::new(f32::NEG_INFINITY, -50.0));
        assert_eq!(clip.max, Vec2::new(f32::INFINITY, 50.0));
    }
}
