//! Explicit offline 1024x768 production-widget evidence, never live acceptance.
//! Run alone with MIR2_I18N_VISUAL_ASSET_ROOT and a fresh
//! MIR2_QUEST_I18N_VISUAL_OUTPUT. No window, connection, account, or save exists.

use super::*;
use crate::native_i18n::{self, Locale};
use crate::native_shell_ui::i18n_visual_tests::{
    capture_i18n, i18n_offscreen_app, i18n_text_layouts, warm_i18n_images,
};
use crate::quest_model::{
    NpcDialogLine, NpcDialogOption, QuestDetailText, QuestObjective, QuestReward, QuestStatus,
};
use crate::read_model::PlayerStats;
use bevy::ui::UiGlobalTransform;
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::PathBuf};

const WIDTH: f32 = 1024.0;
const HEIGHT: f32 = 768.0;
const TRACKER_BOTTOM: f32 = 620.0;

#[derive(Component)]
struct QuestI18nFixtureRoot;

#[derive(Resource, Default)]
struct QuestI18nLayoutFailures(Vec<Value>);

fn quest_from_definition(definition: &Value) -> Quest {
    let id = definition["id"].as_i64().unwrap() as i32;
    let mut objectives = Vec::new();
    for (kind, field) in [("kill", "kills"), ("flag", "flags")] {
        for (index, objective) in definition[field]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            objectives.push(QuestObjective {
                objective_id: format!("{id}:{kind}:{index}"),
                text: objective["message"].as_str().unwrap_or("").to_owned(),
                current: 0,
                target: objective["count"].as_u64().unwrap_or(1) as u32,
            });
        }
    }
    Quest {
        quest_index: id,
        accept_npc_index: definition["startNpcId"].as_u64().map(|n| n as u32),
        finish_npc_index: definition["finishNpcId"].as_u64().map(|n| n as u32),
        title: definition["title"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                native_i18n::format_for(
                    Locale::English,
                    &format!("quest.{id}.title"),
                    "Growth reward",
                    &[],
                )
            }),
        npc_name: None,
        group: Some("Recommended".to_owned()),
        min_level_needed: definition["minLevel"]
            .as_i64()
            .or_else(|| definition["level"].as_i64())
            .unwrap() as i32,
        detail: QuestDetailText::default(),
        status: QuestStatus::InProgress,
        objectives,
        rewards: vec![
            QuestReward::Experience {
                amount: definition["experience"].as_u64().unwrap_or(0) as u32,
            },
            QuestReward::Gold {
                amount: definition["gold"].as_u64().unwrap_or(0) as u32,
            },
        ],
        unknown_text: None,
    }
}

fn fixture_player(quest: &Quest, class: &str) -> PlayerStats {
    PlayerStats {
        level: quest.min_level_needed.max(1) as u32,
        class_name: Some(class.to_owned()),
        hp: 40,
        max_hp: 200,
        mp: 4,
        max_mp: 200,
        gold: 12,
        ..default()
    }
}

fn fixture_journey(tracker: &QuestTracker, player: &PlayerStats) -> JourneyView {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    NewcomerJourneyCatalog::from_guidance(&guidance)
        .derive(
            &guidance,
            tracker,
            &CompletedQuestTracker::default(),
            player,
        )
        .expect("authored newcomer fixture must have a chapter")
}

fn fixture_map() -> crate::big_map::BigMapModel {
    let current = mir2_game_data::crystal_respawn_manifest_ref()
        .maps
        .iter()
        .find(|map| map.map_file_name == "0")
        .expect("real Bichon map")
        .map_index;
    crate::big_map::BigMapModel {
        current_map_index: Some(current),
        ..default()
    }
}

fn render_fixture_tracker(parent: &mut ChildSpawnerCommands, quest: &Quest, class: &str) {
    let tracker = QuestTracker {
        active_quests: vec![quest.clone()],
    };
    let player = fixture_player(quest, class);
    let journey = fixture_journey(&tracker, &player);
    let supplies = crate::quest_supplies::plan(
        &player,
        &InventoryModel::default(),
        None,
        Some(quest.quest_index),
    );
    render_quest_tracker_panel(
        parent,
        &tracker,
        &QuestUiState {
            pinned_primary_quest_index: Some(quest.quest_index),
            ..default()
        },
        Some(&journey),
        &EntityModelSet::default(),
        &MapModel {
            center_x: 328,
            center_y: 264,
            ..default()
        },
        Some(&fixture_map()),
        class,
        &supplies,
    );
}

/// Choose the largest authored card and detail among all 26 quests and the
/// three supported newcomer classes, after translation and real row wrapping.
/// No invented repeated paragraph is used to inflate the stress case.
fn longest_authored_cases() -> ((Quest, &'static str, usize), (Quest, &'static str, usize)) {
    let config = crate::quest_practice::newcomer_config();
    let definitions: Vec<_> = ["quests", "growthRewards"]
        .into_iter()
        .flat_map(|field| config[field].as_array().unwrap())
        .collect();
    assert_eq!(definitions.len(), 26);
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let mut tracker_case = None;
    let mut detail_case = None;
    for definition in definitions {
        let quest = quest_from_definition(definition);
        for class in ["Warrior", "Wizard", "Taoist"] {
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            Commands::new(&mut queue, &world)
                .spawn(Node::default())
                .with_children(|parent| {
                    render_fixture_tracker(parent, &quest, class);
                });
            queue.apply(&mut world);
            let card_rows = world
                .query::<&Text>()
                .iter(&world)
                .map(|text| text.0.lines().count())
                .sum::<usize>();
            if tracker_case
                .as_ref()
                .is_none_or(|(_, _, rows)| card_rows > *rows)
            {
                tracker_case = Some((quest.clone(), class, card_rows));
            }
            let detail_rows = quest_detail_lines(&quest, Some(&guidance), class).len();
            if detail_case
                .as_ref()
                .is_none_or(|(_, _, rows)| detail_rows > *rows)
            {
                detail_case = Some((quest.clone(), class, detail_rows));
            }
        }
    }
    (tracker_case.unwrap(), detail_case.unwrap())
}

fn clear_fixture(app: &mut App) {
    let roots: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<QuestI18nFixtureRoot>>()
        .iter(app.world())
        .collect();
    for root in roots {
        app.world_mut().despawn(root);
    }
}

fn spawn_panel(
    app: &mut App,
    camera: Entity,
    rect: [f32; 4],
    asset: Option<&str>,
    render: impl FnOnce(&mut ChildSpawnerCommands, &AssetServer),
) {
    clear_fixture(app);
    let assets = app.world().resource::<AssetServer>().clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world());
    let mut root = commands.spawn((
        QuestI18nFixtureRoot,
        UiTargetCamera(camera),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect[0]),
            top: Val::Px(rect[1]),
            width: Val::Px(rect[2]),
            height: Val::Px(rect[3]),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ));
    if let Some(asset) = asset {
        root.insert(ImageNode {
            image: assets.load(asset.to_owned()),
            image_mode: NodeImageMode::Stretch,
            ..default()
        });
    }
    root.with_children(|parent| render(parent, &assets));
    queue.apply(app.world_mut());
    warm_i18n_images(app);
}

fn verify_rows(app: &mut App, allow_scrolled_content: bool) -> Vec<Value> {
    let rows = i18n_text_layouts(app);
    assert!(rows.len() >= 3, "actual production text must be present");
    // Keep every locale's evidence before failing, so one narrow title cannot
    // prevent diagnosis of the remaining languages. All former gates remain.
    let failures: Vec<_> = rows
        .iter()
        .filter(|row| {
            row["glyphs"].as_u64().unwrap() == 0
                || row["missingGlyphs"] != 0
                || row["layoutExceedsNode"] != false
                || (!allow_scrolled_content && row["nodeOutsideViewport"] != false)
        })
        .map(|row| json!({"locale": native_i18n::locale().code(), "row": row}))
        .collect();
    app.world_mut().init_resource::<QuestI18nLayoutFailures>();
    app.world_mut()
        .resource_mut::<QuestI18nLayoutFailures>()
        .0
        .extend(failures);
    assert_eq!(
        app.world_mut().query::<&Window>().iter(app.world()).count(),
        0
    );
    rows
}

fn tracker_viewport(app: &mut App) -> (Entity, Value, f32) {
    let world = app.world_mut();
    let (entity, node, transform) = world.query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform), With<multi_guidance::GuidanceViewport>>()
        .single(world).expect("one production tracker scroll viewport");
    let min = transform.translation - node.size / 2.0;
    let max = min + node.size;
    assert!(min.x >= 0.0 && max.x <= WIDTH);
    assert!(
        min.y >= 100.0 && max.y <= TRACKER_BOTTOM + 1.0,
        "tracker overlaps the HUD: {min:?} {max:?}"
    );
    assert!((node.size.x - 304.0).abs() < 1.0);
    let scroll_max = (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor;
    (
        entity,
        json!({"min":[min.x,min.y],"max":[max.x,max.y],"contentSize":[node.content_size.x,node.content_size.y],"maxScroll":scroll_max}),
        scroll_max,
    )
}

fn assert_tracker_bottom_control_visible(app: &mut App, viewport: &Value) {
    let top = viewport["min"][1].as_f64().unwrap() as f32;
    let bottom = viewport["max"][1].as_f64().unwrap() as f32;
    let controls: Vec<_> = app
        .world_mut()
        .query::<(&QuestUiButton, &ComputedNode, &UiGlobalTransform)>()
        .iter(app.world())
        .filter(|(button, _, _)| matches!(button, QuestUiButton::ToggleSupplies))
        .map(|(_, node, transform)| {
            (
                transform.translation.y - node.size.y / 2.0,
                transform.translation.y + node.size.y / 2.0,
            )
        })
        .collect();
    assert_eq!(controls.len(), 1);
    assert!(
        controls[0].0 >= top && controls[0].1 <= bottom + 1.0,
        "last tracker action must be reachable after scrolling: {controls:?}"
    );
}

fn source_dialog(script: &str, section: &str) -> NpcDialogModel {
    let prose: Value = serde_json::from_str(include_str!(
        "../../../../packages/tooling/data/native-i18n/npc-prose-source.json"
    ))
    .unwrap();
    let menus: Value = serde_json::from_str(include_str!(
        "../../../../packages/tooling/data/native-i18n/npc-menu-source.json"
    ))
    .unwrap();
    let mut lines = Vec::new();
    let mut options = Vec::new();
    for (source, is_menu) in [(&prose, false), (&menus, true)] {
        for entry in source["entries"].as_array().unwrap() {
            for reference in entry["references"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|reference| {
                    reference["script"] == script
                        && reference["label"]
                            .as_str()
                            .unwrap()
                            .eq_ignore_ascii_case(section)
                })
            {
                let mut text = entry["en"].as_str().unwrap().to_owned();
                for parameter in entry["parameters"].as_array().unwrap() {
                    assert_eq!(
                        parameter["source"], "<$USERNAME>",
                        "fixture must explicitly model any additional parameter"
                    );
                    text = text.replace(
                        &format!("{{{}}}", parameter["name"].as_str().unwrap()),
                        "Gold",
                    );
                }
                let line = reference["line"].as_u64().unwrap();
                if is_menu {
                    options.push((
                        line,
                        NpcDialogOption {
                            option_id: reference["target"].as_str().unwrap().to_owned(),
                            label: text,
                            enabled: true,
                        },
                    ));
                } else {
                    lines.push((line, NpcDialogLine { text }));
                }
            }
        }
    }
    lines.sort_by_key(|(line, _)| *line);
    options.sort_by_key(|(line, _)| *line);
    assert!(
        !lines.is_empty() && !options.is_empty(),
        "real source section must contain body and links"
    );
    NpcDialogModel {
        is_open: true,
        npc_object_id: Some(1),
        npc_name: Some(
            if script.ends_with("Jane") {
                "Assistant Jane"
            } else {
                "BookStore"
            }
            .into(),
        ),
        lines: lines.into_iter().map(|(_, line)| line).collect(),
        options: options.into_iter().map(|(_, option)| option).collect(),
    }
}

fn longest_bookstore_section() -> (String, NpcDialogModel) {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../packages/tooling/data/native-i18n/npc-menu-source.json"
    ))
    .unwrap();
    let sections: BTreeSet<_> = source["entries"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|entry| entry["references"].as_array().unwrap())
        .filter(|reference| reference["script"] == "BichonProvince/BichonWall/BookStore")
        .map(|reference| reference["label"].as_str().unwrap().to_owned())
        .collect();
    sections
        .into_iter()
        .map(|section| {
            let dialog = source_dialog("BichonProvince/BichonWall/BookStore", &section);
            (section, dialog)
        })
        .max_by_key(|(_, dialog)| npc_dialog_rows(dialog).len())
        .unwrap()
}

fn capture_record(
    app: &mut App,
    target: &Handle<Image>,
    output: &std::path::Path,
    name: &str,
    metadata: Value,
    scrolled: bool,
) -> Value {
    let rows = verify_rows(app, scrolled);
    capture_i18n(app, target, &output.join(format!("{name}.png")));
    json!({"file":format!("{name}.png"),"metadata":metadata,"textRows":rows})
}

#[test]
#[ignore = "explicit offline GPU fixture; run alone with real packaged assets and fresh output"]
fn nine_locale_selected_supply_vendors_have_no_missing_glyphs_offscreen() {
    use crate::quest_supplies::SupplyVendor;
    let asset_root = PathBuf::from(
        std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("explicit real packaged asset root"),
    );
    let output = PathBuf::from(
        std::env::var_os("MIR2_QUEST_I18N_VISUAL_OUTPUT").expect("explicit fresh evidence output"),
    );
    assert!(output.is_absolute());
    fs::create_dir_all(&output).unwrap();
    assert!(!output.join("supply-vendor-i18n-layouts.json").exists());
    let previous_locale = native_i18n::locale();
    let (mut app, target, camera) = i18n_offscreen_app(&asset_root, false);
    app.insert_resource(crate::native_shell::NativeShellModel {
        screen: crate::native_shell::NativeShellScreen::InGame,
        ..default()
    });
    let mut evidence = Vec::new();
    for locale in Locale::ALL {
        native_i18n::activate(locale);
        for vendor in SupplyVendor::ALL {
            // Reproduce the reported level-9 medicine card. The other vendor
            // cases use a Taoist so their actual material rows are present.
            let (class, level, gold) = if vendor == SupplyVendor::Potions {
                ("Warrior", 9, 700)
            } else {
                ("Taoist", 28, 700)
            };
            let player = PlayerStats {
                class_name: Some(class.into()),
                level,
                gold,
                current_weight: 17,
                max_weight: 77,
                current_weight_known: true,
                ..default()
            };
            let inventory = InventoryModel {
                items: vec![crate::inventory::ItemModel {
                    name: "(HP)DrugSmall".into(),
                    quantity: 11,
                    container: 0,
                    slot: 0,
                    unique_id: Some(1),
                    ..default()
                }],
                ..default()
            };
            let supplies = crate::quest_supplies::plan(
                &player,
                &inventory,
                None,
                (vendor != SupplyVendor::Potions).then_some(2_110_021),
            );
            assert!(supplies.rows.iter().any(|row| row.vendor == vendor));
            let state = QuestUiState {
                supply_open: true,
                supply_vendor: Some(vendor),
                ..default()
            };
            spawn_panel(
                &mut app,
                camera,
                [0.0, 100.0, 320.0, 520.0],
                None,
                |parent, _| {
                    multi_guidance::render_supplies(
                        parent,
                        &state,
                        Some(&fixture_map()),
                        &supplies,
                    );
                },
            );
            let (_, viewport, maximum) = tracker_viewport(&mut app);
            assert_eq!(maximum, 0.0, "selected vendor card must fit above the HUD");
            assert_tracker_bottom_control_visible(&mut app, &viewport);
            let name = format!("{}-supply-{}", locale.code(), vendor.npc_name());
            evidence.push(capture_record(
                &mut app,
                &target,
                &output,
                &name,
                json!({"class":class,"level":level,"vendor":vendor.npc_name(),"viewport":viewport}),
                false,
            ));
        }
    }
    native_i18n::activate(previous_locale);
    let failures = &app.world().resource::<QuestI18nLayoutFailures>().0;
    fs::write(
        output.join("supply-vendor-i18n-layouts.json"),
        serde_json::to_vec_pretty(&json!({
            "kind":"offline_production_supply_vendor_widgets","liveAcceptance":false,
            "passed":failures.is_empty(),"layoutFailures":failures,"viewport":[WIDTH,HEIGHT],
            "systemFonts":false,"locales":Locale::ALL.map(Locale::code),"screenshots":evidence,
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(
        failures.is_empty(),
        "selected supply vendor layout failures: {failures:?}"
    );
}

#[test]
#[ignore = "explicit offline GPU fixture; run alone with real packaged assets and fresh output"]
fn nine_locale_longest_quests_and_real_npc_menus_offscreen() {
    let asset_root = PathBuf::from(
        std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("explicit real packaged asset root"),
    );
    let output = PathBuf::from(
        std::env::var_os("MIR2_QUEST_I18N_VISUAL_OUTPUT").expect("explicit fresh evidence output"),
    );
    assert!(output.is_absolute());
    fs::create_dir_all(&output).unwrap();
    assert!(!output.join("quest-i18n-layouts.json").exists());
    let previous_locale = native_i18n::locale();
    let (mut app, target, camera) = i18n_offscreen_app(&asset_root, false);
    app.insert_resource(crate::native_shell::NativeShellModel {
        screen: crate::native_shell::NativeShellScreen::InGame,
        ..default()
    });
    let mut evidence = Vec::new();
    for locale in Locale::ALL {
        native_i18n::activate(locale);
        let code = locale.code();
        let (
            (tracker_quest, tracker_class, tracker_rows),
            (detail_quest, detail_class, detail_rows),
        ) = longest_authored_cases();
        spawn_panel(
            &mut app,
            camera,
            [0.0, 100.0, 320.0, 520.0],
            None,
            |parent, _| render_fixture_tracker(parent, &tracker_quest, tracker_class),
        );
        let (viewport_entity, viewport, maximum) = tracker_viewport(&mut app);
        evidence.push(capture_record(&mut app, &target, &output, &format!("{code}-tracker-top"),
            json!({"questId":tracker_quest.quest_index,"class":tracker_class,"authoredCandidateCount":78,"wrappedRows":tracker_rows,"viewport":viewport}), true));
        app.world_mut()
            .get_mut::<ScrollPosition>(viewport_entity)
            .unwrap()
            .y = maximum;
        warm_i18n_images(&mut app);
        let (_, viewport, _) = tracker_viewport(&mut app);
        assert_tracker_bottom_control_visible(&mut app, &viewport);
        evidence.push(capture_record(
            &mut app,
            &target,
            &output,
            &format!("{code}-tracker-bottom"),
            json!({"scroll":maximum,"viewport":viewport}),
            true,
        ));

        assert!(
            detail_rows > QUEST_DETAIL_LINE_COUNT,
            "fixture must exercise real detail scrolling"
        );
        for (label, top) in [
            ("top", 0),
            (
                "bottom",
                detail_rows.saturating_sub(QUEST_DETAIL_LINE_COUNT),
            ),
        ] {
            spawn_panel(
                &mut app,
                camera,
                [
                    354.0,
                    60.0,
                    QUEST_DETAIL_DESIGN_WIDTH,
                    QUEST_DETAIL_DESIGN_HEIGHT,
                ],
                Some(QUEST_DETAIL_FRAME_ASSET),
                |parent, assets| {
                    render_quest_detail_panel(
                        parent,
                        &detail_quest,
                        &QuestGuidance::from_profile_name("newcomer-v2"),
                        &QuestUiState {
                            detail_scroll_top: top,
                            ..default()
                        },
                        &PendingOperations::default(),
                        Some(assets),
                        &fixture_player(&detail_quest, detail_class),
                    );
                },
            );
            evidence.push(capture_record(&mut app, &target, &output, &format!("{code}-detail-{label}"),
                json!({"questId":detail_quest.quest_index,"class":detail_class,"authoredCandidateCount":78,"totalRows":detail_rows,"top":top}), false));
        }

        let jane = source_dialog("BichonProvince/BorderVillage/Jane", "@Main-1");
        let opaque_rows = npc_dialog_rows(&jane);
        assert!(
            opaque_rows.iter().any(|(text, _, _)| text.contains("Gold")),
            "player name must remain opaque"
        );
        spawn_panel(
            &mut app,
            camera,
            [292.0, 200.0, 440.0, 224.0],
            Some(NPC_DIALOG_FRAME_ASSET),
            |parent, assets| {
                render_dialog_panel(
                    parent,
                    &jane,
                    &NpcDialogNav::default(),
                    &QuestUiState::default(),
                    &PendingOperations::default(),
                    false,
                    Some(assets),
                );
            },
        );
        evidence.push(capture_record(&mut app, &target, &output, &format!("{code}-npc-player-name"),
            json!({"script":"BichonProvince/BorderVillage/Jane","section":"@Main-1","opaquePlayerName":"Gold"}), false));

        let (section, dialog) = longest_bookstore_section();
        let rows = npc_dialog_rows(&dialog);
        let max_top =
            rows.len().saturating_sub(1) / NPC_DIALOG_VISIBLE_ROWS * NPC_DIALOG_VISIBLE_ROWS;
        for (label, top) in [("top", 0), ("bottom", max_top)] {
            spawn_panel(
                &mut app,
                camera,
                [292.0, 200.0, 440.0, 224.0],
                Some(NPC_DIALOG_FRAME_ASSET),
                |parent, assets| {
                    render_dialog_panel(
                        parent,
                        &dialog,
                        &NpcDialogNav::default(),
                        &QuestUiState {
                            dialog_scroll_top: top,
                            ..default()
                        },
                        &PendingOperations::default(),
                        false,
                        Some(assets),
                    );
                },
            );
            let rendered_targets: BTreeSet<_> = app
                .world_mut()
                .query::<&QuestUiButton>()
                .iter(app.world())
                .filter_map(|button| {
                    if let QuestUiButton::SelectNpcDialog { target } = button {
                        Some(target.clone())
                    } else {
                        None
                    }
                })
                .collect();
            let expected: BTreeSet<_> = rows
                .iter()
                .skip(top)
                .take(NPC_DIALOG_VISIBLE_ROWS)
                .filter_map(|(_, target, _)| target.clone())
                .collect();
            assert_eq!(
                rendered_targets, expected,
                "translation must preserve each displayed command target"
            );
            evidence.push(capture_record(&mut app, &target, &output, &format!("{code}-npc-menu-{label}"),
                json!({"script":"BichonProvince/BichonWall/BookStore","section":section,"totalRows":rows.len(),"top":top,"exactCommandTargets":rendered_targets}), false));
        }
    }
    native_i18n::activate(previous_locale);
    let failures = &app.world().resource::<QuestI18nLayoutFailures>().0;
    fs::write(output.join("quest-i18n-layouts.json"), serde_json::to_vec_pretty(&json!({
        "kind":"offline_production_quest_widgets","liveAcceptance":false,"passed":failures.is_empty(),"layoutFailures":failures,"viewport":[WIDTH,HEIGHT],
        "locales":Locale::ALL.map(Locale::code),"selection":"maximum wrapped rows among 26 authored quests × 3 classes per locale",
        "screenshots":evidence,
    })).unwrap()).unwrap();
    assert!(
        failures.is_empty(),
        "translated text layout failures: {failures:?}"
    );
}
