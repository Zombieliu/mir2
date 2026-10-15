use super::*;

use bevy::camera::visibility::VisibilityPlugin;
use bevy::transform::TransformPlugin;

fn assert_visible_sprite_children(world: &World, root: Entity, expected_children: usize) {
    assert!(
        world.get::<Transform>(root).is_some(),
        "fallback root must retain its local transform"
    );
    assert!(
        world.get::<GlobalTransform>(root).is_some(),
        "Transform must install the matching GlobalTransform on the fallback root"
    );
    assert!(
        world.get::<Visibility>(root).is_some(),
        "Sprite children require their fallback root to participate in visibility propagation"
    );
    assert!(
        world.get::<InheritedVisibility>(root).is_some(),
        "Visibility must install InheritedVisibility on the fallback root"
    );

    let children = world
        .get::<Children>(root)
        .expect("fallback root must own its sprite children");
    assert_eq!(children.len(), expected_children);
    for &child in children {
        assert_eq!(
            world
                .get::<ChildOf>(child)
                .expect("sprite child must retain its root")
                .parent(),
            root
        );
        assert!(world.get::<Sprite>(child).is_some());
        assert!(world.get::<Transform>(child).is_some());
        assert!(world.get::<GlobalTransform>(child).is_some());
        assert!(world.get::<Visibility>(child).is_some());
        assert!(world.get::<InheritedVisibility>(child).is_some());
    }
}

#[test]
fn fallback_scene_roots_propagate_transform_and_visibility_to_sprite_children() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins((TransformPlugin, VisibilityPlugin))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
        .init_resource::<RuntimeWorldState>()
        .init_resource::<RuntimeEntityRenderState>()
        .init_resource::<RuntimeMapRenderState>()
        .init_resource::<interpolation::SnapshotBuffer>()
        .init_resource::<motion::EntityMotionTable>()
        .init_resource::<SceneRegistry>()
        .add_systems(
            Update,
            (sync_map_scene, sync_entities, sync_mine_nodes).chain(),
        );

    app.world_mut().resource_mut::<RuntimeWorldState>().snapshot = Some(WorldSnapshot {
        map_file_name: None,
        map_title: Some("fallback hierarchy".to_owned()),
        player_object_id: None,
        selected_object_id: None,
        scene_view: Some(SceneView {
            center: GridPoint { x: 0, y: 0 },
            width: 1,
            height: 1,
        }),
        terrain_patches: Vec::new(),
        decor_objects: vec![DecorObject {
            id: "fallback-rock".to_owned(),
            x: 2,
            y: 3,
            kind: DecorKind::Rock,
        }],
        entities: vec![WorldEntity {
            object_id: "fallback-entity".to_owned(),
            kind: EntityKind::Monster,
            name: "Fallback monster".to_owned(),
            x: 4,
            y: 5,
            direction: None,
            level: None,
            movement_started_ms: None,
            movement_duration_ms: None,
        }],
        mine_nodes: vec![MineNode {
            x: 6,
            y: 7,
            stage: 2,
        }],
        client_time_ms: None,
    });

    app.update();

    let world = app.world();
    let (entity_root, mine_root, map_roots) = {
        let registry = world.resource::<SceneRegistry>();
        (
            registry.entities["fallback-entity"].root,
            registry.mine_nodes[&(6, 7)].root,
            registry.map.spawned.clone(),
        )
    };

    assert_visible_sprite_children(world, entity_root, 5);
    assert_visible_sprite_children(world, mine_root, 2);
    assert_eq!(map_roots.len(), 2, "one terrain tile plus one decor root");

    let mut map_child_counts = map_roots
        .iter()
        .map(|root| {
            let child_count = world
                .get::<Children>(*root)
                .expect("map fallback root children")
                .len();
            assert_visible_sprite_children(world, *root, child_count);
            child_count
        })
        .collect::<Vec<_>>();
    map_child_counts.sort_unstable();
    assert_eq!(map_child_counts, [2, 3]);
}
