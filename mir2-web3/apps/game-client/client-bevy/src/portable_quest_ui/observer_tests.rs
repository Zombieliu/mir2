//! Real PostUpdate observation/visibility regressions without a renderer or IO.
use super::*;
use crate::quest_ui::{QuestUiButton, QuestUiPresentation, QuestUiRoot};
use bevy::camera::visibility::{VisibilityPlugin, VisibilitySystems};
use bevy::ecs::schedule::{NodeId, Schedules};

struct Fixture {
    app: App,
    root: Entity,
    parent: Entity,
    camera: Entity,
}

fn computed(size: Vec2) -> ComputedNode {
    ComputedNode {
        size,
        inverse_scale_factor: 1.0,
        ..default()
    }
}

fn fixture() -> Fixture {
    let mut app = App::new();
    // These are the resources used by the actual VisibilityPlugin's bound
    // calculation systems. No device, window, network, font or assets are loaded.
    app.init_resource::<Assets<bevy::mesh::Mesh>>()
        .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
        .add_plugins(VisibilityPlugin)
        .add_plugins(Mir2PortableQuestUiPlugin);
    let camera = app.world_mut().spawn_empty().id();
    app.insert_resource(QuestUiTargetCamera(camera));
    app.insert_resource(QuestUiPresentation {
        logical_width: 1024.0,
        logical_height: 768.0,
        stage_css_scale: 1.0,
        touch: false,
    });
    app.insert_resource(QuestUiHostContext {
        generation: 7,
        revision: 9,
        host_visible: true,
        in_game: true,
        ..default()
    });
    app.insert_resource(QuestWorldContext {
        stamp: Some(QuestWorldStamp {
            generation: 7,
            revision: 9,
            connection_generation: 2,
            session_generation: 3,
            scene_revision: 4,
            player_object_id: 1000,
            map_file_name: "0".into(),
        }),
        inventory: Some(default()),
        player: Some(default()),
        raw_entities: vec![QuestWorldEntity {
            object_id: 1000,
            kind: EntityKind::SelfPlayer,
            name: "synthetic-player".into(),
            x: 284,
            y: 606,
            direction: None,
            level: Some(1),
            dead: None,
            hp: None,
            max_hp: None,
        }],
        ..default()
    });
    let root = app
        .world_mut()
        .spawn((
            QuestUiRoot,
            Node::default(),
            computed(Vec2::new(1024.0, 768.0)),
            bevy::ui::UiTargetCamera(camera),
            Visibility::Inherited,
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((Node::default(), Visibility::Inherited, ChildOf(root)))
        .id();
    // The root is already painted when an Update rebuild replaces card children.
    // Only PostUpdate runs: the production plugin's Startup/Update render tree
    // is deliberately not invoked alongside this bounded layout fixture.
    app.world_mut().run_schedule(PostUpdate);
    assert!(app.world().get::<InheritedVisibility>(root).unwrap().get());
    assert!(app
        .world()
        .get::<InheritedVisibility>(parent)
        .unwrap()
        .get());
    Fixture {
        app,
        root,
        parent,
        camera,
    }
}

fn button(f: &mut Fixture, action: QuestUiButton, y: f32) -> Entity {
    f.app
        .world_mut()
        .spawn((
            Button,
            action,
            Node::default(),
            // Fixed computed geometry represents the completed Layout stage; the
            // actual observer still reads ComputedNode/UiGlobalTransform and all
            // real ancestry, camera, bounds and source checks.
            computed(Vec2::new(80.5, 25.25)),
            bevy::ui::UiGlobalTransform::from_xy(140.25, y),
            Visibility::Inherited,
            ChildOf(f.parent),
        ))
        .id()
}

fn observed(f: &Fixture) -> &QuestWorldControlRects {
    f.app.world().resource::<QuestWorldControlRects>()
}

fn assert_current_empty(f: &Fixture) {
    assert_eq!(
        observed(f).stamp,
        f.app.world().resource::<QuestWorldContext>().stamp
    );
    assert!(observed(f).rects.is_empty());
}

#[derive(Resource, Default)]
struct BeforePropagation(Vec<QuestWorldControlRects>);

fn capture_before_propagation(
    rects: Res<QuestWorldControlRects>,
    mut before: ResMut<BeforePropagation>,
) {
    before.0.push(rects.clone());
}

#[test]
fn production_observer_requires_both_layout_and_visibility_schedule_barriers() {
    let f = fixture();
    let schedules = f.app.world().resource::<Schedules>();
    let graph = schedules.get(PostUpdate).unwrap().graph();
    let observers = graph
        .systems_in_set(QuestWorldControlObservationSet::Observe.intern())
        .unwrap();
    assert_eq!(
        observers.len(),
        1,
        "inspect the actual production plugin registration"
    );
    let observer = NodeId::System(*observers.first().unwrap());
    for required in [
        bevy::ui::UiSystems::Layout.intern(),
        VisibilitySystems::VisibilityPropagate.intern(),
    ] {
        let before = NodeId::Set(graph.system_sets.get_key(required).unwrap());
        assert!(
            graph.dependency().graph().contains_edge(before, observer),
            "the production observer must explicitly wait for {required:?}"
        );
    }
}

#[test]
fn rebuilt_buttons_publish_same_frame_after_real_visibility_and_old_phase_is_empty() {
    let mut f = fixture();
    f.app.init_resource::<BeforePropagation>().add_systems(
        PostUpdate,
        // Negative control: the same real observer at its old legal execution
        // phase sees current roots and Layout, but not newly propagated children.
        (observe_world_control_rects, capture_before_propagation)
            .chain()
            .after(bevy::ui::UiSystems::Layout)
            .before(VisibilitySystems::VisibilityPropagate),
    );
    for revision in 10..43 {
        let previous = f
            .app
            .world()
            .get::<Children>(f.parent)
            .map(|children| children.to_vec())
            .unwrap_or_default();
        for child in previous {
            f.app.world_mut().despawn(child);
        }
        f.app
            .world_mut()
            .resource_mut::<QuestUiHostContext>()
            .revision = revision;
        f.app
            .world_mut()
            .resource_mut::<QuestWorldContext>()
            .stamp
            .as_mut()
            .unwrap()
            .revision = revision;
        let children = [
            button(
                &mut f,
                QuestUiButton::SelectQuest {
                    quest_index: 2_110_001,
                },
                140.125,
            ),
            button(&mut f, QuestUiButton::OpenDestinationMap, 180.125),
            button(&mut f, QuestUiButton::ToggleSupplies, 220.125),
        ];
        for child in children {
            assert!(!f
                .app
                .world()
                .get::<InheritedVisibility>(child)
                .unwrap()
                .get());
            assert!(
                f.app
                    .world()
                    .get::<ComputedNode>(child)
                    .unwrap()
                    .size()
                    .min_element()
                    > 0.0
            );
        }
        f.app.world_mut().run_schedule(PostUpdate);
        let before = f
            .app
            .world()
            .resource::<BeforePropagation>()
            .0
            .last()
            .unwrap();
        assert_eq!(
            before.stamp,
            f.app.world().resource::<QuestWorldContext>().stamp
        );
        assert!(
            before.rects.is_empty(),
            "the old Layout-only phase loses all three freshly rebuilt controls"
        );
        let actual = observed(&f);
        assert_eq!(actual.stamp, before.stamp);
        assert_eq!(
            (actual.logical_width, actual.logical_height),
            (1024.0, 768.0)
        );
        let mut types = actual
            .rects
            .iter()
            .map(|r| r.action_type)
            .collect::<Vec<_>>();
        types.sort_unstable();
        assert_eq!(
            types,
            ["openDestinationMap", "questLocalUi", "toggleSupplies"]
        );
        assert!(actual
            .rects
            .iter()
            .all(|r| r.left == 100.0 && r.width == 80.5 && r.height == 25.25));
        assert!(children.into_iter().all(|child| f
            .app
            .world()
            .get::<InheritedVisibility>(child)
            .unwrap()
            .get()));
    }
    assert_eq!(f.app.world().resource::<BeforePropagation>().0.len(), 33);
}

#[test]
fn hidden_display_none_wrong_camera_and_stale_source_never_publish_controls() {
    let mut f = fixture();
    button(&mut f, QuestUiButton::OpenDestinationMap, 140.125);
    f.app.world_mut().run_schedule(PostUpdate);
    assert_eq!(observed(&f).rects.len(), 1);

    *f.app.world_mut().get_mut::<Visibility>(f.parent).unwrap() = Visibility::Hidden;
    f.app.world_mut().run_schedule(PostUpdate);
    assert_current_empty(&f);
    *f.app.world_mut().get_mut::<Visibility>(f.parent).unwrap() = Visibility::Inherited;
    f.app.world_mut().run_schedule(PostUpdate);
    assert_eq!(observed(&f).rects.len(), 1);

    f.app.world_mut().get_mut::<Node>(f.parent).unwrap().display = Display::None;
    f.app.world_mut().run_schedule(PostUpdate);
    assert_current_empty(&f);
    f.app.world_mut().get_mut::<Node>(f.parent).unwrap().display = Display::Flex;
    f.app.world_mut().run_schedule(PostUpdate);
    assert_eq!(observed(&f).rects.len(), 1);

    let foreign = f.app.world_mut().spawn_empty().id();
    f.app
        .world_mut()
        .get_mut::<bevy::ui::UiTargetCamera>(f.root)
        .unwrap()
        .0 = foreign;
    f.app.world_mut().run_schedule(PostUpdate);
    assert!(observed(&f).stamp.is_none());
    assert!(observed(&f).rects.is_empty());
    f.app
        .world_mut()
        .get_mut::<bevy::ui::UiTargetCamera>(f.root)
        .unwrap()
        .0 = f.camera;
    f.app.world_mut().run_schedule(PostUpdate);
    assert_eq!(observed(&f).rects.len(), 1);

    f.app
        .world_mut()
        .resource_mut::<QuestUiHostContext>()
        .revision += 1;
    f.app.world_mut().run_schedule(PostUpdate);
    assert!(observed(&f).stamp.is_none());
    assert!(observed(&f).rects.is_empty());
    f.app
        .world_mut()
        .resource_mut::<QuestUiHostContext>()
        .revision -= 1;
    f.app.world_mut().run_schedule(PostUpdate);
    assert_eq!(observed(&f).rects.len(), 1);

    *f.app.world_mut().get_mut::<Visibility>(f.root).unwrap() = Visibility::Hidden;
    f.app.world_mut().run_schedule(PostUpdate);
    assert!(observed(&f).stamp.is_none());
    assert!(observed(&f).rects.is_empty());
}

#[test]
fn local_card_blockers_do_not_reclassify_other_controls_or_wire_actions() {
    let mut f = fixture();
    for (index, action) in [
        QuestUiButton::SelectQuest {
            quest_index: 2_110_001,
        },
        QuestUiButton::MakePrimary {
            quest_index: 2_110_003,
        },
        QuestUiButton::PrepareQuestFinish {
            quest_index: 2_110_001,
        },
        QuestUiButton::AcceptQuest {
            npc_index: 10,
            quest_index: 2_110_001,
        },
        QuestUiButton::FinishQuest {
            quest_index: 2_110_001,
            selected_item_index: 0,
        },
        QuestUiButton::AbandonQuest {
            quest_index: 2_110_001,
        },
        QuestUiButton::ShareQuest {
            quest_index: 2_110_001,
        },
        QuestUiButton::TrackQuest {
            quest_index: 2_110_001,
        },
    ]
    .into_iter()
    .enumerate()
    {
        button(&mut f, action, 140.125 + index as f32 * 40.0);
    }
    f.app.world_mut().run_schedule(PostUpdate);
    assert_eq!(observed(&f).rects.len(), 3);
    assert!(observed(&f)
        .rects
        .iter()
        .all(|r| r.action_type == "questLocalUi"));
    let encoded = serde_json::to_value(observed(&f)).unwrap();
    assert_eq!(encoded["version"], 1);
    for rect in encoded["rects"].as_array().unwrap() {
        assert_eq!(rect["type"], "questLocalUi");
        assert_eq!(rect.as_object().unwrap().len(), 5);
        assert!(rect.get("questIndex").is_none());
        assert!(rect.get("actionType").is_none());
    }
    assert!(f
        .app
        .world_mut()
        .resource_mut::<QuestPresentationActionQueue>()
        .take()
        .is_none());
    assert!(f
        .app
        .world_mut()
        .resource_mut::<QuestUiIntentQueue>()
        .drain_intents()
        .is_empty());
}
