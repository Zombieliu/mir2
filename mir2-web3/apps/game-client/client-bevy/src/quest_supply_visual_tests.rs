//! Explicit offline UI fixture: no window, gateway, account data or input.
//! Run this ignored test alone after ordinary tests. It uses the production
//! tracker/supply renderers, Bevy layout/text shaping and an offscreen GPU image.

use super::*;
use crate::{
    big_map::BigMapModel,
    inventory::{InventoryModel, ItemModel},
    quest_model::{QuestDetailText, QuestObjective, QuestStatus},
    quest_supplies::{SupplyPlan, SupplyVendor},
    read_model::{PlayerStats, PlayerWeights},
};
use bevy::{
    camera::RenderTarget,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    text::TextLayoutInfo,
    ui::UiGlobalTransform,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const COLUMN_WIDTH: f32 = 336.0;
const FIXTURE_TOP: f32 = 32.0;

#[derive(Component)]
struct FixtureRoot;

#[derive(Component)]
struct FixtureCaption;

#[derive(Component)]
struct FixtureCard;

#[derive(Resource, Default)]
struct CapturedImage(Option<Image>);

#[derive(Clone, Copy)]
struct Case {
    title: &'static str,
    class: &'static str,
    map: &'static str,
    vendor: Option<SupplyVendor>,
    quest: bool,
}

fn map_index(file_name: &str) -> i32 {
    mir2_game_data::crystal_respawn_manifest_ref()
        .maps
        .iter()
        .find(|map| map.map_file_name == file_name)
        .unwrap_or_else(|| panic!("fixture map missing: {file_name}"))
        .map_index
}

fn shortage_plan(class: &str) -> SupplyPlan {
    // Full ordinary bag of unrelated items: no fabricated carried supplies.
    // Stats/slots exist only in this test and are never saved or sent anywhere.
    let player = PlayerStats {
        level: 28,
        class_name: Some(class.to_owned()),
        hp: 40,
        max_hp: 200,
        mp: 4,
        max_mp: 200,
        gold: 12,
        current_weight: 100,
        current_weight_known: true,
        max_weight: 100,
        weights: Some(PlayerWeights {
            bag: 100,
            wear: 10,
            hand: 4,
        }),
        ..default()
    };
    let mut inventory = InventoryModel {
        gold: player.gold,
        ..default()
    };
    inventory.items = (0..inventory.bag_slot_capacity())
        .map(|slot| ItemModel {
            key: format!("visual-fixture-unrelated-{slot}"),
            name: "视觉夹具占位物品".to_owned(),
            container: 0,
            slot: u32::from(slot),
            quantity: 1,
            ..default()
        })
        .collect();
    let result = crate::quest_supplies::plan(&player, &inventory, None, Some(2_110_021));
    assert!(result.bag_full && result.gold < result.estimated_cost);
    assert!(result.rows.iter().all(|row| row.held == 0));
    assert!(result
        .rows
        .iter()
        .any(|row| row.item_name == "(MP)DrugLarge"));
    if class == "Taoist" {
        assert!(result.needs_taoist_material_guidance);
        assert!(result
            .rows
            .iter()
            .any(|row| row.vendor == SupplyVendor::Poison));
    }
    result
}

fn quest_fixture() -> (QuestTracker, JourneyView) {
    let quest = Quest {
        quest_index: 2_110_021,
        accept_npc_index: None,
        finish_npc_index: None,
        title: "Prove your class tactics".to_owned(),
        npc_name: None,
        group: Some("Recommended".to_owned()),
        min_level_needed: 28,
        detail: QuestDetailText::default(),
        status: QuestStatus::InProgress,
        objectives: vec![
            QuestObjective {
                objective_id: "visual-kill".to_owned(),
                text: "Defeat 3 WoomaFighter.".to_owned(),
                current: 1,
                target: 3,
            },
            QuestObjective {
                objective_id: "visual-practice".to_owned(),
                text: "Complete your class practice".to_owned(),
                current: 0,
                target: 1,
            },
        ],
        rewards: Vec::new(),
        unknown_text: None,
    };
    let journey = JourneyView {
        chapter_id: "graduation".to_owned(),
        chapter_title: "Wooma Graduation".to_owned(),
        level_range: "28–30".to_owned(),
        completed_count: Some(1),
        quest_count: Some(4),
        goal: String::new(),
        reward_summary: String::new(),
        class_hint: None,
        graduated: false,
        graduation: None,
        next: None,
        optional: Vec::new(),
    };
    (
        QuestTracker {
            active_quests: vec![quest],
        },
        journey,
    )
}

fn offscreen_app() -> (App, Handle<Image>, Entity) {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::app::TerminalCtrlCHandlerPlugin>(),
    );
    app.insert_resource(ClearColor(Color::srgb(0.10, 0.12, 0.14)))
        .init_resource::<CapturedImage>();
    let initialization = Instant::now();
    while app.plugins_state() != bevy::app::PluginsState::Ready {
        assert!(
            initialization.elapsed() < Duration::from_secs(30),
            "offscreen plugin initialization timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Bgra8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            Msaa::Off,
        ))
        .id();
    (app, target, camera)
}

fn caption(parent: &mut ChildSpawnerCommands, text: &str, rect: (f32, f32, f32)) {
    parent.spawn((
        FixtureCaption,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.0),
            top: Val::Px(rect.1),
            width: Val::Px(rect.2),
            ..default()
        },
        Text::new(text),
        TextFont {
            font: FontSource::Family("Microsoft YaHei".into()),
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.88, 0.95)),
    ));
}

fn spawn_cases(app: &mut App, camera: Entity, cases: &[Case; 3]) {
    let previous: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<FixtureRoot>>()
        .iter(app.world())
        .collect();
    for entity in previous {
        app.world_mut().despawn(entity);
    }
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world());
    let mut roots = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let supplies = shortage_plan(case.class);
        let current = map_index(case.map);
        if let Some(vendor) = case.vendor {
            assert!(
                vendor.route(current).is_some(),
                "fixture must exercise a real authored route: {}",
                case.title
            );
        }
        let state = QuestUiState {
            supply_open: !case.quest,
            supply_vendor: case.vendor,
            pinned_primary_quest_index: Some(2_110_021),
            ..default()
        };
        let big_map = BigMapModel {
            current_map_index: Some(current),
            ..default()
        };
        let mut root = commands.spawn((
            FixtureRoot,
            UiTargetCamera(camera),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(index as f32 * COLUMN_WIDTH),
                top: Val::Px(FIXTURE_TOP),
                width: Val::Px(COLUMN_WIDTH),
                height: Val::Px(HEIGHT as f32 - FIXTURE_TOP),
                ..default()
            },
        ));
        roots.push(root.id());
        root.with_children(|parent| {
            if case.quest {
                let (tracker, journey) = quest_fixture();
                assert!(multi_guidance::render(
                    parent,
                    &tracker,
                    &state,
                    Some(&journey),
                    &EntityModelSet::default(),
                    &MapModel {
                        center_x: 324,
                        center_y: 291,
                        ..default()
                    },
                    Some(&big_map),
                    case.class,
                    &supplies
                ));
            } else {
                multi_guidance::render_supplies(parent, &state, Some(&big_map), &supplies);
            }
        });
        commands
            .spawn((
                FixtureRoot,
                UiTargetCamera(camera),
                Node {
                    width: Val::Px(WIDTH as f32),
                    height: Val::Px(30.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ))
            .with_children(|parent| {
                caption(
                    parent,
                    case.title,
                    (index as f32 * COLUMN_WIDTH + 8.0, 12.0, 318.0),
                )
            });
    }
    commands
        .spawn((
            FixtureRoot,
            UiTargetCamera(camera),
            Node {
                width: Val::Px(WIDTH as f32),
                height: Val::Px(HEIGHT as f32),
                position_type: PositionType::Absolute,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(680.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.4, 0.5)),
            ));
            caption(
                parent,
                "离线构造场景 · 真实补给/任务组件 · 未连接角色或服务器",
                (16.0, 704.0, 992.0),
            );
            caption(
                parent,
                "法师/道士 28 级：库存不足、金币 12、负重 100/100、背包已满",
                (16.0, 732.0, 992.0),
            );
        });
    queue.apply(app.world_mut());
    for root in roots {
        let children: Vec<_> = app.world().get::<Children>(root).unwrap().iter().collect();
        assert_eq!(children.len(), 1, "renderer should create exactly one card");
        app.world_mut().entity_mut(children[0]).insert(FixtureCard);
    }
}

fn step(app: &mut App) {
    app.update();
    app.sub_app(RenderApp)
        .world()
        .resource::<RenderDevice>()
        .poll(PollType::wait_indefinitely())
        .expect("offscreen GPU completion");
}

fn verify_layout(app: &mut App) -> serde_json::Value {
    let mut rows = Vec::new();
    let mut bounds: [Vec<(f32, f32, String)>; 3] = std::array::from_fn(|_| Vec::new());
    let world = app.world_mut();
    assert_eq!(
        world.query::<&Window>().iter(world).count(),
        0,
        "the fixture must never create a window"
    );
    for (text, node, transform, layout) in world.query_filtered::<(&Text, &ComputedNode, &UiGlobalTransform, &TextLayoutInfo), Without<FixtureCaption>>().iter(world) {
        assert!(!layout.glyphs.is_empty(), "text must actually rasterize: {}", text.0);
        assert!(layout.size.x <= node.size.x + 1.0, "text wider than its reserved node: {:?} {:?} {}", layout.size, node.size, text.0);
        assert!(layout.size.y <= node.size.y + 1.0, "text taller than its reserved row: {:?} {:?} {}", layout.size, node.size, text.0);
        let center = transform.affine().translation;
        let min = center - node.size * 0.5;
        let max = center + node.size * 0.5;
        let column = (center.x / COLUMN_WIDTH).floor() as usize;
        assert!(column < 3);
        let column_left = column as f32 * COLUMN_WIDTH;
        assert!(min.x >= column_left + 18.0 && max.x <= column_left + 303.0, "text outside card content: {} {min:?} {max:?}", text.0);
        assert!(max.y <= 680.0, "card text reaches the reserved bottom area: {} {max:?}", text.0);
        bounds[column].push((min.y, max.y, text.0.clone()));
        rows.push(serde_json::json!({ "text": text.0, "column": column, "min": [min.x,min.y], "max": [max.x,max.y], "glyphs": layout.glyphs.len(), "textSize": [layout.size.x,layout.size.y] }));
    }
    assert!(
        rows.len() > 20,
        "fixture should contain real rendered cards"
    );
    for column in &mut bounds {
        column.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in column.windows(2) {
            assert!(
                pair[0].1 <= pair[1].0 + 0.5,
                "text rows overlap: {:?} / {:?}",
                pair[0],
                pair[1]
            );
        }
    }
    let cards: Vec<_> = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<FixtureCard>>()
        .iter(world)
        .map(|(node, transform)| {
            let center = transform.affine().translation;
            assert!((node.size.x - 304.0).abs() < 0.5);
            assert!(
                center.y + node.size.y * 0.5 <= 680.0,
                "card overlaps the reserved bottom area"
            );
            serde_json::json!({"size": [node.size.x,node.size.y],"center": [center.x,center.y]})
        })
        .collect();
    assert_eq!(cards.len(), 3);
    serde_json::json!({"kind":"offline_production_ui_fixture", "liveAcceptance":false, "cards":cards, "textRows":rows})
}

#[test]
#[ignore = "explicit offscreen GPU screenshots; run alone after ordinary tests"]
fn caster_supplies_and_chinese_quest_cards_offscreen() {
    let output = std::env::var_os("MIR2_SUPPLY_VISUAL_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mir2-caster-supplies-visual"));
    std::fs::create_dir_all(&output).unwrap();
    let sheets = [
        (
            "01-overview",
            [
                Case {
                    title: "法师 28 级 · 补给总览",
                    class: "Wizard",
                    map: "0",
                    vendor: None,
                    quest: false,
                },
                Case {
                    title: "道士 28 级 · 补给总览",
                    class: "Taoist",
                    map: "0",
                    vendor: None,
                    quest: false,
                },
                Case {
                    title: "法师 28 级 · 中文主线",
                    class: "Wizard",
                    map: "0",
                    vendor: None,
                    quest: true,
                },
            ],
        ),
        (
            "02-shop-routes",
            [
                Case {
                    title: "红蓝药 · 比奇城到商人",
                    class: "Wizard",
                    map: "0",
                    vendor: Some(SupplyVendor::Potions),
                    quest: false,
                },
                Case {
                    title: "符与卷轴 · 比奇城到商人",
                    class: "Taoist",
                    map: "0",
                    vendor: Some(SupplyVendor::General),
                    quest: false,
                },
                Case {
                    title: "毒粉 · 比奇城到药材店",
                    class: "Taoist",
                    map: "0",
                    vendor: Some(SupplyVendor::Poison),
                    quest: false,
                },
            ],
        ),
        (
            "03-poison-and-taoist",
            [
                Case {
                    title: "毒粉 · 药材店到制药室",
                    class: "Taoist",
                    map: "0108",
                    vendor: Some(SupplyVendor::Poison),
                    quest: false,
                },
                Case {
                    title: "毒粉 · 制药室到商人",
                    class: "Taoist",
                    map: "0109",
                    vendor: Some(SupplyVendor::Poison),
                    quest: false,
                },
                Case {
                    title: "道士 28 级 · 中文主线",
                    class: "Taoist",
                    map: "0",
                    vendor: None,
                    quest: true,
                },
            ],
        ),
    ];
    let (mut app, target, camera) = offscreen_app();
    for (filename, cases) in sheets {
        spawn_cases(&mut app, camera, &cases);
        for _ in 0..12 {
            step(&mut app);
        }
        let layout = verify_layout(&mut app);
        app.world_mut().resource_mut::<CapturedImage>().0 = None;
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |captured: On<ScreenshotCaptured>, mut output: ResMut<CapturedImage>| {
                    output.0 = Some(captured.image.clone());
                },
            );
        let started = Instant::now();
        while app.world().resource::<CapturedImage>().0.is_none() {
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "offscreen screenshot timed out"
            );
            step(&mut app);
            std::thread::sleep(Duration::from_millis(5));
        }
        let image = app
            .world_mut()
            .resource_mut::<CapturedImage>()
            .0
            .take()
            .unwrap()
            .try_into_dynamic()
            .expect("GPU image to PNG")
            .to_rgba8();
        assert_eq!(image.dimensions(), (WIDTH, HEIGHT));
        assert!(
            image
                .pixels()
                .filter(|pixel| pixel[0] > 150 && pixel[1] > 150 && pixel[2] < 180)
                .count()
                > 300,
            "screenshot should contain actual gold UI labels/borders"
        );
        image
            .save(output.join(format!("{filename}.png")))
            .expect("write fixture PNG");
        std::fs::write(
            output.join(format!("{filename}.json")),
            serde_json::to_vec_pretty(&layout).unwrap(),
        )
        .unwrap();
        eprintln!(
            "offline caster UI fixture saved: {}",
            output.join(format!("{filename}.png")).display()
        );
    }
}
