//! Periodic task presentation, ordinary NPC authorization and route regressions.
//! No account, save, reward or live server is changed by these fixtures.
use super::*;
use crate::big_map::{BigMapModel, BigMapPoint};
use crate::entities::{EntityKind, EntityModel};
use crate::native_i18n::{self, Locale};
use crate::quest_model::{NpcDialogOption, QuestDetailText, QuestObjective, QuestReward, QuestStatus};

fn task(id: i32, status: QuestStatus) -> Quest {
    let definition = mir2_game_data::periodic_quests::quest(id).unwrap();
    let info = definition.info(12_345, definition.gold);
    let objectives_complete = matches!(status, QuestStatus::ReadyToTurnIn | QuestStatus::Completed);
    Quest {
        quest_index: id, accept_npc_index: Some(2180), finish_npc_index: Some(2180),
        title: info.name, npc_name: Some("Bichon Task Steward".into()), group: Some(info.group),
        min_level_needed: definition.min_level, status,
        detail: QuestDetailText { description_lines: info.description,
            task_description_lines: info.task_description, return_description_lines: info.return_description,
            completion_description_lines: info.completion_description, time_limit: None },
        objectives: definition.kills.iter().enumerate().map(|(index, kill)| QuestObjective {
            objective_id: format!("{id}:{index}"),
            text: format!("Kill {} {}/{}", kill.monster, if objectives_complete { kill.count } else { 0 },kill.count),
            current: if objectives_complete { kill.count } else { 0 },
            target: kill.count,
        }).collect(),
        rewards: vec![QuestReward::Experience { amount: 12_345 }, QuestReward::Gold { amount: definition.gold }],
        unknown_text: None,
    }
}

fn offer(npc: u32, id: i32, finish: bool) -> NpcDialogModel {
    NpcDialogModel { is_open: true, npc_object_id: Some(npc), options: vec![NpcDialogOption {
        option_id: format!("@quest:{}:{id}", if finish { "finish" } else { "accept" }),
        label: "Task operation".into(), enabled: true,
    }], ..default() }
}

fn entity(npc: u32, x: i32, y: i32) -> EntityModel {
    EntityModel { object_id: npc.to_string(), kind: EntityKind::Npc, name: "Task Steward".into(),
        x, y, direction: None, level: None }
}

fn finish_app(npc: u32) -> App {
    let destination = mir2_game_data::periodic_quests::npc(npc).unwrap();
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() });
    app.init_resource::<NativePlayerUiState>();
    app.insert_resource(QuestTracker { active_quests: vec![task(92010, QuestStatus::ReadyToTurnIn)] });
    app.insert_resource(QuestGuidance::from_profile_name("newcomer-v2"));
    app.init_resource::<NpcDialogModel>();
    app.init_resource::<NpcDialogNav>();
    app.insert_resource(QuestUiState { detail_quest_index: Some(92010), ..default() });
    app.init_resource::<QuestUiIntentQueue>();
    app.init_resource::<PendingOperations>();
    app.insert_resource(MapModel { center_x: destination.x + 2, center_y: destination.y, ..default() });
    app.insert_resource(BigMapModel { current_map_index: Some(destination.map_index), ..default() });
    app.insert_resource(EntityModelSet { entities: vec![entity(npc, destination.x, destination.y)] });
    app.add_systems(Update, process_quest_ui_input);
    app
}

fn click_finish(app: &mut App) {
    app.world_mut().spawn((Button, QuestUiButton::PrepareQuestFinish { quest_index: 92010 }, Interaction::Pressed));
    app.update();
}

fn drain(app: &mut App) -> Vec<QuestUiIntent> {
    app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents()
}

#[test]
fn periodic_accept_in_both_towns_requires_this_npcs_actual_server_offer() {
    let tracker = QuestTracker { active_quests: vec![task(92001, QuestStatus::NotStarted)] };
    for npc in [2180, 2181] {
        let offered = offer(npc, 92001, false);
        assert_eq!(npc_available_quest_indices(&offered, &tracker, None), vec![92001]);
        assert!(npc_list_accept_is_current(&[92001], &offered, &tracker, npc, 92001));
        for stale in [offer(npc, 92002, false), offer(npc, 92001, true), offer(24, 92001, false),
            NpcDialogModel { is_open: false, ..offered.clone() },
            NpcDialogModel { options: vec![], ..offered.clone() }] {
            assert!(!npc_list_accept_is_current(&[92001], &stale, &tracker, npc, 92001));
        }
        let active = QuestTracker { active_quests: vec![task(92001, QuestStatus::InProgress)] };
        assert!(!npc_list_accept_is_current(&[92001], &offered, &active, npc, 92001));
    }
}

#[test]
fn periodic_actual_npc_list_accept_button_sends_the_offering_towns_id_once() {
    for npc in [2180,2181] {
        let mut app = finish_app(npc);
        let quest = task(92001,QuestStatus::NotStarted);
        let dialog = offer(npc,quest.quest_index,false);
        app.insert_resource(QuestTracker { active_quests:vec![quest.clone()] });
        app.insert_resource(dialog.clone());
        app.insert_resource(QuestUiState { npc_quest_selected_index:Some(quest.quest_index),..default() });
        let mut queue = bevy::ecs::world::CommandQueue::default();
        Commands::new(&mut queue,app.world()).spawn(Node::default()).with_children(|parent| {
            render_npc_quest_list_panel(parent,&[&quest],&dialog,&QuestGuidance::from_profile_name("newcomer-v2"),
                &QuestUiState { npc_quest_selected_index:Some(quest.quest_index),..default() },&PendingOperations::default(),None,
                &crate::read_model::PlayerStats {level:10,..default()});
        });
        queue.apply(app.world_mut());
        let button = app.world_mut().query::<(Entity,&QuestUiButton)>().iter(app.world())
            .find_map(|(entity,button)|matches!(button,QuestUiButton::AcceptNpcQuest {npc_index,quest_index}
                if *npc_index==npc && *quest_index==quest.quest_index).then_some(entity)).expect("actual NPC-specific Accept control");
        *app.world_mut().get_mut::<Interaction>(button).unwrap()=Interaction::Pressed;
        app.update();
        assert_eq!(drain(&mut app),vec![QuestUiIntent::AcceptQuest {npc_index:npc,quest_index:quest.quest_index}]);
        *app.world_mut().get_mut::<Interaction>(button).unwrap()=Interaction::None;
        app.update();
        *app.world_mut().get_mut::<Interaction>(button).unwrap()=Interaction::Pressed;
        app.update();
        assert!(drain(&mut app).is_empty(),"pending acceptance cannot be sent twice");
        assert_eq!(app.world().resource::<QuestTracker>().active_quests[0].status,QuestStatus::NotStarted,
            "the client waits for the authoritative accept acknowledgement");
    }
}

#[test]
fn periodic_detail_finish_selects_current_town_waits_for_offer_and_submits_once() {
    for npc in [2180, 2181] {
        let mut app = finish_app(npc);
        click_finish(&mut app);
        assert_eq!(drain(&mut app), vec![QuestUiIntent::InteractQuestNpc { quest_index: 92010, npc_object_id: npc }]);
        app.insert_resource(offer(if npc == 2180 { 2181 } else { 2180 }, 92010, true));
        app.update();
        assert!(drain(&mut app).is_empty(), "another town cannot satisfy this dialog request");
        app.insert_resource(offer(npc, 92010, true));
        app.update();
        assert_eq!(drain(&mut app), vec![QuestUiIntent::FinishQuest { quest_index: 92010, selected_item_index: -1 }]);
        app.update();
        assert!(drain(&mut app).is_empty());
        assert_eq!(app.world().resource::<QuestTracker>().active_quests[0].status, QuestStatus::ReadyToTurnIn,
            "a client click cannot complete a quest or grant its rewards");
    }
}

#[test]
fn periodic_pending_finish_rejects_map_transfer_distance_missing_npc_and_completion() {
    for changed in ["epoch", "map", "distance", "missing-npc", "completed", "closed-detail"] {
        let mut app = finish_app(2181);
        click_finish(&mut app); drain(&mut app);
        match changed {
            "epoch" => app.world_mut().resource_mut::<BigMapModel>().reset_epoch += 1,
            "map" => app.world_mut().resource_mut::<BigMapModel>().current_map_index = Some(1),
            "distance" => app.world_mut().resource_mut::<MapModel>().center_x = 900,
            "missing-npc" => app.world_mut().resource_mut::<EntityModelSet>().entities.clear(),
            "completed" => app.world_mut().resource_mut::<QuestTracker>().active_quests[0].status = QuestStatus::Completed,
            _ => app.world_mut().resource_mut::<QuestUiState>().close_detail(),
        }
        app.insert_resource(offer(2181, 92010, true)); app.update();
        assert!(drain(&mut app).is_empty(), "{changed}");
        assert!(app.world().resource::<QuestUiState>().pending_turn_in.is_none(), "{changed}");
    }
}

#[test]
fn periodic_ready_guidance_prefers_current_town_then_fewest_legal_entrances() {
    let ready = task(92020, QuestStatus::ReadyToTurnIn);
    for npc in &mir2_game_data::periodic_quests::catalog().npcs {
        let destination = turn_in::destination_on_map(&ready, Some(npc.map_index)).unwrap();
        assert_eq!(destination.object_id, npc.object_id);
        assert_eq!((destination.x, destination.y), (npc.x, npc.y));
    }
    for file in ["D001", "D601", "D711", "D501"] {
        let current = mir2_game_data::crystal_map_respawns_ref(file).unwrap().map_index;
        let destination = turn_in::destination_on_map(&ready, Some(current)).unwrap();
        let chosen_hops = match route::resolve(Some(current), &[destination.map_index]) {
            route::QuestRoute::NextStep(step) => step.remaining_hops,
            _ => panic!("expected a legal town-return route from {file}"),
        };
        assert!(mir2_game_data::periodic_quests::catalog().npcs.iter().all(|npc|
            match route::resolve(Some(current), &[npc.map_index]) {
                route::QuestRoute::NextStep(step) => chosen_hops <= step.remaining_hops,
                route::QuestRoute::Unavailable => true,
                _ => false,
            }));
    }
    assert!(turn_in::destination_on_map(&ready, Some(i32::MAX)).is_none());
}

#[test]
fn periodic_hunt_guidance_uses_canonical_species_and_unfinished_server_counters() {
    let mut weekly = task(92015, QuestStatus::InProgress);
    weekly.objectives[0].text = "任意翻譯文字".into();
    weekly.objectives[1].text = "Unrelated display text".into();
    let map = mir2_game_data::crystal_map_respawns_ref("D601").unwrap().map_index;
    let tracker = QuestTracker { active_quests: vec![weekly.clone()] };
    let regions = crate::quest_hunt_regions::active_hunt_regions_for_primary(&tracker, map, BigMapPoint { x: 40, y: 27 }, Some(92015));
    assert_eq!(regions.len(), 2);
    assert!(regions.iter().all(|region| region.primary && region.radius == 8));
    assert!(crate::crystal_ui::quest_targets::quest_targets_monster(&weekly, "WhimperingBee"));
    assert!(crate::crystal_ui::quest_targets::quest_targets_monster(&weekly, "GiantWorm"));
    assert!(!crate::crystal_ui::quest_targets::quest_targets_monster(&weekly, "Worm"));
    weekly.objectives[0].current = weekly.objectives[0].target;
    assert!(!crate::crystal_ui::quest_targets::quest_targets_monster(&weekly, "WhimperingBee"));
    let tracker = QuestTracker { active_quests: vec![weekly.clone()] };
    assert_eq!(crate::quest_hunt_regions::active_hunt_regions(&tracker, map, BigMapPoint { x: 40, y: 27 })[0].monster_index, 154);
    assert!(crate::quest_hunt_regions::active_hunt_regions(&tracker, 1, BigMapPoint { x: 40, y: 27 }).is_empty());
    weekly.objectives.clear();
    assert!(!crate::crystal_ui::quest_targets::quest_targets_monster(&weekly, "GiantWorm"));
    assert!(crate::quest_destination::active_target_map_indices(&weekly).is_empty());
}

#[test]
fn periodic_task_npc_route_validates_status_coordinate_id_map_and_epoch() {
    let ready = task(92010, QuestStatus::ReadyToTurnIn);
    let tracker = QuestTracker { active_quests: vec![ready] };
    let state = QuestUiState { pinned_primary_quest_index: Some(92010), ..default() };
    let map = BigMapModel { current_map_index: Some(172), reset_epoch: 4, ..default() };
    let intent = QuestRouteNavigationIntent { target: QuestRouteTarget::TaskNpc { object_id: 2181 },
        quest_index: 92010, reset_epoch: 4, map_index: 172, x: 334, y: 330 };
    assert!(quest_route_intent_is_current(intent, &tracker, &state, None, Some(&map)));
    for forged in [QuestRouteNavigationIntent { x: 333, ..intent },
        QuestRouteNavigationIntent { map_index: 1, ..intent },
        QuestRouteNavigationIntent { reset_epoch: 3, ..intent },
        QuestRouteNavigationIntent { target: QuestRouteTarget::TaskNpc { object_id: 24 }, ..intent },
        QuestRouteNavigationIntent { quest_index: 1234, ..intent }] {
        assert!(!quest_route_intent_is_current(forged, &tracker, &state, None, Some(&map)));
    }
    let completed = QuestTracker { active_quests: vec![task(92010, QuestStatus::Completed)] };
    assert!(!intent.matches_task_npc_destination(&completed));
}

#[test]
fn periodic_daily_and_weekly_tabs_keep_old_active_band_and_do_not_mix_main_or_side() {
    let guidance = QuestGuidance::from_profile_name("newcomer-v2");
    let tracker = QuestTracker { active_quests: vec![
        task(92001, QuestStatus::InProgress), task(92006, QuestStatus::NotStarted),
        task(92009, QuestStatus::InProgress), task(92010, QuestStatus::ReadyToTurnIn),
        task(92002, QuestStatus::Completed),
    ] };
    let ids = |tab| guided_diary_quests(&tracker, Some(&guidance), None, tab).into_iter()
        .map(|quest| quest.quest_index).collect::<Vec<_>>();
    assert_eq!(ids(GuidedDiaryTab::Daily), vec![92001, 92006]);
    assert_eq!(ids(GuidedDiaryTab::Weekly), vec![92010, 92009]);
    assert_eq!(ids(GuidedDiaryTab::Ready), vec![92010]);
    assert!(ids(GuidedDiaryTab::Main).is_empty());
    assert!(ids(GuidedDiaryTab::Side).is_empty());
}

#[test]
fn periodic_nine_language_catalog_covers_actual_server_copy_without_losing_reset_rules() {
    let catalog: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../packages/game-data/data/native-i18n/periodic.json")).unwrap();
    assert_eq!(catalog["schema"], 1);
    let entries = catalog["entries"].as_array().unwrap();
    let actual_kill_keys = entries.iter().filter_map(|entry| entry["key"].as_str()
        .filter(|key| key.starts_with("periodic.kill.")).map(str::to_owned))
        .collect::<std::collections::BTreeSet<_>>();
    let expected_kill_keys = mir2_game_data::periodic_quests::catalog().quests.iter()
        .flat_map(|definition| definition.kills.iter())
        .map(|kill| format!("periodic.kill.{}.{}", kill.monster_index, kill.count))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_kill_keys, expected_kill_keys, "kill aliases use only current authored counts");
    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || {
            for definition in &mir2_game_data::periodic_quests::catalog().quests {
                let quest = task(definition.id, QuestStatus::InProgress);
                let title = crate::player_text::quest_title(quest.quest_index, "MISSING TITLE");
                let description = crate::player_text::quest_description(quest.quest_index, "MISSING DESCRIPTION");
                assert!(!title.contains("MISSING") && !description.contains("MISSING"));
                assert!(description.contains("UTC+8"), "{} {}", locale.code(), quest.quest_index);
                assert_eq!(description.lines().count(), 5, "{} {}", locale.code(), quest.quest_index);
                if locale == Locale::English { assert_eq!(description, quest.detail.description_lines.join("\n")); }
                let summary = native_i18n::for_locale(locale, &format!("periodic.summary.{}", definition.id), "MISSING SUMMARY");
                assert_eq!(description.lines().next(), Some(summary.as_str()));
                assert_eq!(native_i18n::tr(&definition.summary), summary, "current server summary alias resolves");
                assert_eq!(native_i18n::tr(&quest.detail.description_lines.join("\n")), description,
                    "current full server description alias resolves");
                let tasks = crate::player_text::quest_section(quest.quest_index, "task", &quest.detail.task_description_lines);
                assert_eq!(tasks.len(), definition.kills.len());
                for (kill, text) in definition.kills.iter().zip(&tasks) {
                    let key = format!("periodic.kill.{}.{}", kill.monster_index, kill.count);
                    let entry = entries.iter().find(|entry| entry["key"].as_str() == Some(key.as_str())).unwrap();
                    let expected = entry[locale.code()].as_str().unwrap();
                    assert_eq!(text, expected, "{} {} task and kill label agree", locale.code(), definition.id);
                    let count = text.rsplit_once('\u{00d7}').expect("authored target count").1.trim().parse::<u32>().unwrap();
                    assert_eq!(count, kill.count, "{} {} counts survive localization exactly", locale.code(), definition.id);
                    let alias = format!("Kill {} {}", kill.count, kill.monster);
                    assert_eq!(entry["aliases"].as_array().unwrap().len(), 1);
                    assert_eq!(entry["aliases"][0].as_str(), Some(alias.as_str()), "current server task alias");
                    assert_eq!(native_i18n::tr(&alias), expected, "{} {} task alias resolves", locale.code(), definition.id);
                }
                for (index,kill) in definition.kills.iter().enumerate() {
                    let localized_name=native_i18n::for_locale(locale,&format!("content.monster.{}.name",kill.monster_index),"MISSING SPECIES");
                    let label=crate::player_text::quest_objective_label(quest.quest_index,index,"Unrelated wire display text 5/999");
                    assert!(label.contains(&localized_name),"authored identity selects the localized species");
                    assert!(!label.contains("wire display")&&!label.contains("5/999"));
                    let progress=quest_objective_detail_text(quest.quest_index,index,&QuestObjective {
                        current:17,target:100,text:"Unrelated wire display text 5/999".into(),objective_id:index.to_string(),
                    });
                    assert!(progress.contains("17 / 100"),"progress comes from the authoritative counters");
                    assert!(!progress.contains("5/999"));
                }
                for prefix in ["accept","complete"] {
                    let source=format!("{} {}",if prefix=="accept" {"Accept"}else{"Complete"},definition.title);
                    assert_eq!(native_i18n::npc_text(&source),native_i18n::for_locale(locale,
                        &format!("periodic.{prefix}.{}",definition.id),"MISSING NPC CAPTION"));
                }
                for class in ["Warrior", "Wizard", "Taoist"] {
                    let lines = quest_detail_lines(&quest, Some(&QuestGuidance::from_profile_name("newcomer-v2")), class);
                    assert!(!lines.is_empty());
                    assert!(lines.iter().all(|line| !line.text.contains('\u{fffd}')));
                }
            }
            for entry in entries {
                let key = entry["key"].as_str().unwrap();
                let expected = entry[locale.code()].as_str().unwrap();
                assert!(!expected.trim().is_empty() && !expected.contains('\u{fffd}'));
                assert_eq!(native_i18n::for_locale(locale, key, "MISSING KEY"), expected);
            }
        });
    }
}

#[test]
fn periodic_tabs_and_eight_rows_stay_above_the_existing_pagination_controls() {
    for locale in Locale::ALL {
        native_i18n::with_locale(locale, || {
            let guidance = QuestGuidance::from_profile_name("newcomer-v2");
            let tracker = QuestTracker { active_quests: mir2_game_data::periodic_quests::catalog().quests.iter()
                .map(|definition| task(definition.id, QuestStatus::InProgress)).collect() };
            for tab in [GuidedDiaryTab::Daily, GuidedDiaryTab::Weekly] {
                let mut world = World::new();
                let mut queue = bevy::ecs::world::CommandQueue::default();
                Commands::new(&mut queue, &world).spawn(Node::default()).with_children(|parent| {
                    render_guided_quest_diary_panel(parent, &tracker, &guidance, None,
                        &QuestUiState { diary_tab: tab, ..default() }, None);
                });
                queue.apply(&mut world);
                assert!(world.query::<(&QuestDiaryRow, &Node)>().iter(&world).all(|(_, node)| {
                    matches!((node.top, node.height), (Val::Px(y), Val::Px(height)) if y >= 133.0 && y + height <= 408.0)
                }));
                let tabs = world.query::<(&QuestUiButton, &Node)>().iter(&world)
                    .filter(|(button, _)| matches!(button, QuestUiButton::SelectGuidedDiaryTab(_)))
                    .collect::<Vec<_>>();
                assert_eq!(tabs.len(), 5);
                assert!(tabs.iter().all(|(_, node)| matches!((node.left, node.top, node.width, node.height),
                    (Val::Px(x), Val::Px(y), Val::Px(width), Val::Px(height)) if x >= 15.0 && x + width <= 307.0 && y + height <= 86.0)));
            }
        });
    }
}

#[test]
fn periodic_ready_tracker_names_either_town_and_emits_the_current_towns_npc_route() {
    native_i18n::with_locale(Locale::English, || {
        let tracker = QuestTracker { active_quests: vec![task(92020, QuestStatus::ReadyToTurnIn)] };
        let guidance = QuestGuidance::from_profile_name("newcomer-v2");
        let player = crate::read_model::PlayerStats { level: 40, class_name: Some("Wizard".into()), ..default() };
        let journey = NewcomerJourneyCatalog::from_guidance(&guidance)
            .derive(&guidance, &tracker, &CompletedQuestTracker::default(), &player).unwrap();
        for npc in &mir2_game_data::periodic_quests::catalog().npcs {
            let big_map = BigMapModel { current_map_index: Some(npc.map_index), ..default() };
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            Commands::new(&mut queue, &world).spawn(Node::default()).with_children(|parent| {
                assert!(multi_guidance::render(parent, &tracker,
                    &QuestUiState { pinned_primary_quest_index: Some(92020), ..default() },
                    Some(&journey), &EntityModelSet::default(), &MapModel::default(), Some(&big_map), "Wizard",
                    &crate::quest_supplies::plan(&player, &InventoryModel::default(), None, Some(92020))));
            });
            queue.apply(&mut world);
            let texts = world.query::<&Text>().iter(&world)
                .map(|text| text.0.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>();
            assert!(texts.iter().any(|text| text.contains("Return to either town Task Steward.")));
            assert!(!texts.iter().any(|text| text.contains(&journey.chapter_title)), "a daily/weekly card has its own heading");
            assert!(world.query::<&QuestUiButton>().iter(&world).any(|button| {
                matches!(button, QuestUiButton::NavigateQuestRoute(intent)
                    if intent.target == QuestRouteTarget::TaskNpc { object_id: npc.object_id }
                        && intent.map_index == npc.map_index && intent.x == npc.x && intent.y == npc.y)
            }));
        }
    });
}

/// Explicit offline GPU evidence exercises the same widgets and bundled fonts
/// as the host. It never opens a window, connection, preference file or save.
mod visual {
    use super::*;
    use crate::native_shell_ui::i18n_visual_tests::{capture_i18n, i18n_offscreen_app, i18n_text_layouts, warm_i18n_images};
    use crate::quest_model::NpcDialogLine;
    use crate::read_model::PlayerStats;
    use bevy::ui::UiGlobalTransform;
    use serde_json::{json, Value};
    use std::{collections::BTreeSet, fs, path::{Path, PathBuf}};

    #[derive(Component)]
    struct PeriodicFixtureRoot;

    fn panel(app: &mut App, camera: Entity, rect: [f32; 4], asset: Option<&str>,
        render: impl FnOnce(&mut ChildSpawnerCommands, &AssetServer)) {
        let roots = app.world_mut().query_filtered::<Entity, With<PeriodicFixtureRoot>>()
            .iter(app.world()).collect::<Vec<_>>();
        for root in roots { app.world_mut().despawn(root); }
        let assets = app.world().resource::<AssetServer>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world());
        let mut root = commands.spawn((PeriodicFixtureRoot, UiTargetCamera(camera), Node {
            position_type: PositionType::Absolute, left: Val::Px(rect[0]), top: Val::Px(rect[1]),
            width: Val::Px(rect[2]), height: Val::Px(rect[3]), overflow: Overflow::clip(), ..default()
        }, BackgroundColor(PANEL_BG)));
        if let Some(asset) = asset { root.insert(ImageNode { image: assets.load(asset.to_owned()),
            image_mode: NodeImageMode::Stretch, ..default() }); }
        root.with_children(|parent| render(parent, &assets));
        queue.apply(app.world_mut());
        warm_i18n_images(app);
    }

    fn record(app: &mut App, target: &Handle<Image>, output: &Path, name: &str,
        metadata: Value, scrolled: bool, failures: &mut Vec<Value>) -> Value {
        let rows = i18n_text_layouts(app);
        assert!(rows.len() >= 3, "production widgets must have real laid-out text");
        failures.extend(rows.iter().filter(|row| row["glyphs"].as_u64().unwrap() == 0
            || row["missingGlyphs"] != 0 || row["layoutExceedsNode"] != false
            || (!scrolled && row["nodeOutsideViewport"] != false))
            .map(|row| json!({"image":name,"locale":native_i18n::locale().code(),"row":row})));
        assert_eq!(app.world_mut().query::<&Window>().iter(app.world()).count(), 0);
        capture_i18n(app, target, &output.join(format!("{name}.png")));
        json!({"image":format!("{name}.png"),"metadata":metadata,"textRows":rows})
    }

    fn render_tracker(app: &mut App, camera: Entity, quest: &Quest, npc: &mir2_game_data::periodic_quests::PeriodicNpc) {
        let tracker = QuestTracker { active_quests: vec![quest.clone()] };
        let player = PlayerStats { level: 40, class_name: Some("Wizard".into()), hp:200,max_hp:200,mp:200,max_mp:200,gold:100_000,..default() };
        let guidance = QuestGuidance::from_profile_name("newcomer-v2");
        let journey = NewcomerJourneyCatalog::from_guidance(&guidance)
            .derive(&guidance, &tracker, &CompletedQuestTracker::default(), &player).unwrap();
        let big_map = BigMapModel { current_map_index: Some(npc.map_index), ..default() };
        panel(app, camera, [0.0,100.0,320.0,520.0], None, |parent,_| {
            render_quest_tracker_panel(parent, &tracker, &QuestUiState { pinned_primary_quest_index:Some(quest.quest_index), ..default() },
                Some(&journey), &EntityModelSet::default(), &MapModel { center_x:npc.x+6,center_y:npc.y+6,..default() },
                Some(&big_map), "Wizard", &crate::quest_supplies::plan(&player,&InventoryModel::default(),None,Some(quest.quest_index)));
        });
        let viewport = app.world_mut().query_filtered::<(Entity,&ComputedNode,&UiGlobalTransform), With<multi_guidance::GuidanceViewport>>()
            .single(app.world()).map(|(entity,node,transform)| (entity,node.size,node.content_size,node.inverse_scale_factor,transform.translation)).unwrap();
        let top = viewport.4.y - viewport.1.y / 2.0;
        let bottom = top + viewport.1.y;
        assert!(top >=100.0 && bottom <=621.0, "tracker remains above the HUD");
        let scroll = (viewport.2.y - viewport.1.y).max(0.0) * viewport.3;
        if scroll > 0.0 {
            app.world_mut().get_mut::<ScrollPosition>(viewport.0).unwrap().y = scroll;
            warm_i18n_images(app);
        }
        let controls = app.world_mut().query::<(&QuestUiButton,&ComputedNode,&UiGlobalTransform)>()
            .iter(app.world()).filter(|(button,_,_)| matches!(button,QuestUiButton::ToggleSupplies))
            .map(|(_,node,transform)| (transform.translation.y-node.size.y/2.0,transform.translation.y+node.size.y/2.0)).collect::<Vec<_>>();
        assert_eq!(controls.len(),1);
        assert!(controls[0].0 >= top && controls[0].1 <=bottom+1.0, "the last tracker control stays reachable");
        assert!(app.world_mut().query::<&QuestUiButton>().iter(app.world()).any(|button|
            matches!(button,QuestUiButton::NavigateQuestRoute(intent)
                if intent.target == QuestRouteTarget::TaskNpc { object_id:npc.object_id })));
    }

    fn steward_dialog() -> NpcDialogModel {
        let npc = mir2_game_data::periodic_quests::npc(2181).unwrap();
        let mut options = mir2_game_data::periodic_quests::catalog().quests.iter()
            .filter(|quest|quest.min_level==35).map(|quest|NpcDialogOption {
                option_id:format!("@quest:accept:{}",quest.id),label:format!("Accept {}",quest.title),enabled:true,
            }).collect::<Vec<_>>();
        options.push(NpcDialogOption {option_id:"@Exit".into(),label:"Exit".into(),enabled:true});
        NpcDialogModel { is_open:true,npc_object_id:Some(npc.object_id),npc_name:Some(npc.name.clone()),
            lines:["Daily and Weekly Tasks","Bichon and Mongchon share these tasks and rewards.",
                "Daily reset: 00:00 server time (UTC+8).","Weekly reset: Monday 00:00 server time (UTC+8).",
                "Tasks unlock at level 10. Claimed tasks return after reset."]
                .into_iter().map(|text|NpcDialogLine {text:text.into()}).collect(),options }
    }

    #[test]
    #[ignore = "explicit offline GPU fixture; requires packaged assets and fresh evidence output"]
    fn periodic_nine_locale_diary_details_and_steward_widgets_offscreen() {
        let asset_root = PathBuf::from(std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("explicit packaged asset root"));
        let output = PathBuf::from(std::env::var_os("MIR2_PERIODIC_I18N_VISUAL_OUTPUT").expect("explicit fresh output"));
        assert!(output.is_absolute());
        fs::create_dir_all(&output).unwrap();
        assert!(!output.join("periodic-i18n-layouts.json").exists(), "preserve earlier evidence");
        let previous = native_i18n::locale();
        let (mut app,target,camera) = i18n_offscreen_app(&asset_root,false);
        app.insert_resource(NativeShellModel {screen:NativeShellScreen::InGame,..default()});
        let guidance = QuestGuidance::from_profile_name("newcomer-v2");
        let mut captures = Vec::new();
        let mut failures = Vec::new();
        for locale in Locale::ALL {
            native_i18n::activate(locale);
            let code = locale.code();
            let tracker = QuestTracker { active_quests: (92016..=92020).map(|id|task(id,QuestStatus::InProgress)).collect() };
            for (tab,name) in [(GuidedDiaryTab::Daily,"daily"),(GuidedDiaryTab::Weekly,"weekly")] {
                panel(&mut app,camera,[25.0,40.0,QUEST_DIARY_DESIGN_WIDTH,QUEST_DIARY_DESIGN_HEIGHT],
                    Some(QUEST_DIARY_FRAME_ASSET),|parent,assets| {
                        render_guided_quest_diary_panel(parent,&tracker,&guidance,None,&QuestUiState {diary_tab:tab,..default()},Some(assets));
                    });
                captures.push(record(&mut app,&target,&output,&format!("{code}-diary-{name}"),
                    json!({"tab":name,"levelBand":[35,50],"serverQuestIds":[92016,92017,92018,92019,92020]}),false,&mut failures));
            }
            let detail = mir2_game_data::periodic_quests::catalog().quests.iter()
                .map(|definition|task(definition.id,QuestStatus::InProgress))
                .max_by_key(|quest|quest_detail_lines(quest,Some(&guidance),"Taoist").len()).unwrap();
            let total = quest_detail_lines(&detail,Some(&guidance),"Taoist").len();
            for (name,scroll) in [("top",0),("bottom",total.saturating_sub(QUEST_DETAIL_LINE_COUNT))] {
                panel(&mut app,camera,[354.0,60.0,QUEST_DETAIL_DESIGN_WIDTH,QUEST_DETAIL_DESIGN_HEIGHT],
                    Some(QUEST_DETAIL_FRAME_ASSET),|parent,assets| {
                        render_quest_detail_panel(parent,&detail,&guidance,&QuestUiState {detail_scroll_top:scroll,..default()},
                            &PendingOperations::default(),Some(assets),&PlayerStats {level:detail.min_level_needed as u32,class_name:Some("Taoist".into()),..default()});
                    });
                captures.push(record(&mut app,&target,&output,&format!("{code}-detail-{name}"),
                    json!({"questId":detail.quest_index,"rows":total,"scrollTop":scroll,"selection":"longest actual periodic detail in this locale"}),false,&mut failures));
            }
            for npc in &mir2_game_data::periodic_quests::catalog().npcs {
                let ready = task(92020,QuestStatus::ReadyToTurnIn);
                render_tracker(&mut app,camera,&ready,npc);
                captures.push(record(&mut app,&target,&output,&format!("{code}-ready-{}",npc.object_id),
                    json!({"questId":ready.quest_index,"npcId":npc.object_id,"map":npc.map,"mapIndex":npc.map_index}),true,&mut failures));
            }
            let dialog = steward_dialog();
            let rows = npc_dialog_rows(&dialog);
            let last_page = rows.len().saturating_sub(1) / NPC_DIALOG_VISIBLE_ROWS * NPC_DIALOG_VISIBLE_ROWS;
            for (name,scroll) in [("top",0),("bottom",last_page)] {
                panel(&mut app,camera,[292.0,200.0,440.0,224.0],Some(NPC_DIALOG_FRAME_ASSET),|parent,assets| {
                    render_dialog_panel(parent,&dialog,&NpcDialogNav::default(),&QuestUiState {dialog_scroll_top:scroll,..default()},
                        &PendingOperations::default(),false,Some(assets));
                });
                let expected = rows.iter().skip(scroll).take(NPC_DIALOG_VISIBLE_ROWS)
                    .filter_map(|(_,target,_)|target.clone()).collect::<BTreeSet<_>>();
                let actual = app.world_mut().query::<&QuestUiButton>().iter(app.world()).filter_map(|button|
                    if let QuestUiButton::SelectNpcDialog {target}=button {Some(target.clone())}else{None}).collect::<BTreeSet<_>>();
                assert_eq!(actual,expected,"translated menu labels preserve actual NPC commands");
                captures.push(record(&mut app,&target,&output,&format!("{code}-npc-menu-{name}"),
                    json!({"npcId":2181,"rows":rows.len(),"scrollTop":scroll,"targets":actual}),false,&mut failures));
            }
            for (npc_id,status,name) in [(2181,QuestStatus::NotStarted,"accept"),(2180,QuestStatus::ReadyToTurnIn,"finish")] {
                let quest = task(92020,status.clone());
                let offered = offer(npc_id,quest.quest_index,status==QuestStatus::ReadyToTurnIn);
                panel(&mut app,camera,[354.0,60.0,QUEST_LIST_DESIGN_WIDTH,QUEST_LIST_DESIGN_HEIGHT],Some(QUEST_LIST_FRAME_ASSET),|parent,assets| {
                    render_npc_quest_list_panel(parent,&[&quest],&offered,&guidance,
                        &QuestUiState {npc_quest_selected_index:Some(quest.quest_index),..default()},
                        &PendingOperations::default(),Some(assets),&PlayerStats {level:40,class_name:Some("Warrior".into()),..default()});
                });
                assert!(app.world_mut().query::<&QuestUiButton>().iter(app.world()).any(|button|match button {
                    QuestUiButton::AcceptNpcQuest {npc_index,quest_index} =>status==QuestStatus::NotStarted&&*npc_index==npc_id&&*quest_index==quest.quest_index,
                    QuestUiButton::FinishQuest {quest_index,..}=>status==QuestStatus::ReadyToTurnIn&&*quest_index==quest.quest_index,
                    _=>false,
                }));
                captures.push(record(&mut app,&target,&output,&format!("{code}-npc-list-{name}"),
                    json!({"npcId":npc_id,"questId":quest.quest_index,"status":name,"rewardExperience":12345,"rewardGold":quest.rewards[1].label()}),false,&mut failures));
            }
        }
        native_i18n::activate(previous);
        fs::write(output.join("periodic-i18n-layouts.json"),serde_json::to_vec_pretty(&json!({
            "kind":"offline_production_periodic_widgets","liveAcceptance":false,"passed":failures.is_empty(),
            "assetRoot":asset_root,"systemFonts":false,"viewport":[1024,768],"locales":Locale::ALL.map(Locale::code),
            "catalogSource":"config/quest-guidance/daily-weekly-v1.json","layoutFailures":failures,"captures":captures,
        })).unwrap()).unwrap();
        assert!(failures.is_empty(),"periodic widget layout failures: {failures:?}");
    }
}
