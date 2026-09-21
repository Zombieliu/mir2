//! Local task prioritization; all objectives and destinations remain read-only.
use super::*;
use crate::big_map::BigMapModel;
use crate::quest_model::QuestStatus;

/// Manual active choice wins until authoritative completion/removal. Journey's
/// recommended next step (including accept/turn-in) otherwise remains primary.
pub fn primary_quest_index(
    tracker: &QuestTracker,
    state: &QuestUiState,
    journey: Option<&JourneyView>,
) -> Option<i32> {
    state.pinned_primary_quest_index
        .filter(|id| tracker.active_quests.iter().any(|q| q.quest_index == *id && q.status.is_active()))
        .or_else(|| journey.and_then(|j| j.next.as_ref()).map(|step| step.quest_id)
            .filter(|id| tracker.active_quests.iter().any(|q| q.quest_index == *id && !q.status.is_finished())))
        .or_else(|| visible_tracker_quests(tracker, state).first().map(|q| q.quest_index))
        .or_else(|| tracker.active_quests.iter().find(|q| q.status.is_active()).map(|q| q.quest_index))
}

#[derive(Debug, PartialEq, Eq)]
enum Place {
    Current { label: String, distance: u32 },
    Other(String),
    Unknown,
}

fn place(quest: &Quest, tracker: &QuestTracker, entities: &EntityModelSet, map: &MapModel, big_map: Option<&BigMapModel>) -> Place {
    if let Some(target) = nearest_quest_monster(tracker, Some(quest.quest_index), entities, map.center_x, map.center_y) {
        return Place::Current {
            label: format!("{} ({},{}) · {} 格", target.entity.name, target.entity.x, target.entity.y, target.distance),
            distance: target.distance,
        };
    }
    let npc_index = match quest.status {
        QuestStatus::ReadyToTurnIn => quest.finish_npc_index,
        QuestStatus::NotStarted => quest.accept_npc_index,
        _ => None,
    };
    if let (Some(index), Some(big_map)) = (npc_index.filter(|id| *id > 0), big_map) {
        for entry in big_map.maps.values() {
            if let Some(npc) = entry.info.npcs.iter().find(|npc| u32::try_from(npc.index) == Ok(index)) {
                let label = format!("{} · {} ({},{})", entry.info.title, npc.name, npc.location.x, npc.location.y);
                return if big_map.current_map_index == Some(entry.map_index) {
                    Place::Current { label, distance: map.center_x.abs_diff(npc.location.x).max(map.center_y.abs_diff(npc.location.y)) }
                } else { Place::Other(label) };
            }
        }
    }
    if quest.status == QuestStatus::InProgress {
        if let Some(current_map) = big_map.and_then(|model| model.current_map_index) {
            // The authored respawn area is a fallback when no relevant monster
            // is presently visible. Scope it to this one authoritative quest:
            // unrelated active objectives must not make a task look nearby.
            let single = QuestTracker {
                active_quests: vec![quest.clone()],
            };
            if let Some(region) = crate::quest_hunt_regions::active_hunt_regions(
                &single,
                current_map,
                crate::big_map::BigMapPoint {
                    x: map.center_x,
                    y: map.center_y,
                },
            )
            .into_iter()
            .min_by_key(|region| {
                map.center_x
                    .abs_diff(region.center.x)
                    .max(map.center_y.abs_diff(region.center.y))
            }) {
                let distance = map
                    .center_x
                    .abs_diff(region.center.x)
                    .max(map.center_y.abs_diff(region.center.y));
                return Place::Current {
                    label: format!(
                        "狩猎区域 · {} ({},{}) · {} 格",
                        region.name, region.center.x, region.center.y, distance
                    ),
                    distance,
                };
            }
        }
        if let Some(label) = authored_other_map(quest.quest_index, big_map.and_then(|m| m.current_map_index)) {
            return Place::Other(label);
        }
    }
    Place::Unknown
}

fn authored_other_map(quest_index: i32, current_map: Option<i32>) -> Option<String> {
    static CONFIG: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    let current_map = current_map?;
    let config = CONFIG.get_or_init(|| serde_json::from_str(include_str!(
        "../../../../config/quest-guidance/newcomer-journey-v2.json")).expect("bundled V2 guidance"));
    let definition = config["quests"].as_array()?.iter().find(|q| q["id"].as_i64() == Some(i64::from(quest_index)))?;
    let files = definition["maps"].as_array()?;
    let maps = &mir2_game_data::crystal_respawn_manifest_ref().maps;
    let targets = maps.iter().filter(|map| files.iter().any(|f| f.as_str() == Some(map.map_file_name.as_str()))).collect::<Vec<_>>();
    if targets.iter().any(|map| map.map_index == current_map) { return None; }
    let labels = targets.into_iter().map(|map| map.map_title.as_str()).collect::<Vec<_>>();
    (!labels.is_empty()).then(|| labels.join(" / "))
}

fn nearby_indices(primary: i32, tracker: &QuestTracker, entities: &EntityModelSet, map: &MapModel, big_map: Option<&BigMapModel>) -> Vec<i32> {
    let mut candidates = tracker.active_quests.iter()
        .filter(|q| q.quest_index != primary && q.status.is_active())
        .filter_map(|q| match place(q, tracker, entities, map, big_map) {
            Place::Current { distance, .. } => Some((distance, q.quest_index)),
            _ => None,
        }).collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.into_iter().take(2).map(|(_, id)| id).collect()
}

fn objective_lines(quest: &Quest) -> Vec<String> {
    quest.objectives.iter().map(|o| format!("{}  {}/{}", o.text, o.current, o.target)).collect()
}

fn line(parent: &mut ChildSpawnerCommands, text: impl Into<String>, color: Color) {
    parent.spawn((
        Node { width: Val::Percent(100.0), min_height: Val::Px(18.0), flex_shrink: 0.0, ..default() },
        Text::new(text),
        TextFont { font: FontSource::Family("Microsoft YaHei".into()), font_size: FontSize::Px(12.0), ..default() },
        TextColor(color), TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
    ));
}

fn button(parent: &mut ChildSpawnerCommands, text: &str, action: QuestUiButton) {
    parent.spawn((
        Button, action, QuestUiButtonVisual { enabled: true },
        Node { min_height: Val::Px(25.0), width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            align_items: AlignItems::Center, justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() },
        BackgroundColor(BUTTON_BG), FocusPolicy::Block,
    )).with_children(|node| line(node, text, PANEL_HIGHLIGHT));
}

pub(super) fn render(parent: &mut ChildSpawnerCommands, tracker: &QuestTracker, state: &QuestUiState,
    journey: Option<&JourneyView>, entities: &EntityModelSet, map: &MapModel, big_map: Option<&BigMapModel>) -> bool {
    let Some(primary) = primary_quest_index(tracker, state, journey) else { return false; };
    let Some(quest) = tracker.active_quests.iter().find(|q| q.quest_index == primary) else { return false; };
    let nearby = nearby_indices(primary, tracker, entities, map, big_map);
    parent.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(8.0), top: Val::Px(16.0), width: Val::Px(304.0),
            flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)), ..default() },
        BackgroundColor(Color::srgba(0.035, 0.03, 0.02, 0.90)), BorderColor::all(PANEL_HIGHLIGHT),
    )).with_children(|card| {
        if let Some(journey) = journey {
            line(card, format!("{} · {}", journey.chapter_title, journey.progress_label()), PANEL_TEXT);
        }
        line(card, format!("当前任务 · {}", quest.title), PANEL_HIGHLIGHT);
        for objective in objective_lines(quest) { line(card, objective, Color::WHITE); }
        let next = journey.and_then(|j| j.next.as_ref()).filter(|s| s.quest_id == primary);
        if quest.status == QuestStatus::ReadyToTurnIn {
            line(card, next.map(|s| s.action.clone()).unwrap_or_else(|| quest.npc_name.as_ref()
                .map(|n| format!("返回 {n} 交付任务")).unwrap_or_else(|| "打开任务详情查看交付方式".into())), FEEDBACK_OK);
        } else if quest.status == QuestStatus::NotStarted {
            line(card, next.map(|s| s.action.as_str()).unwrap_or("查看详情领取任务"), FEEDBACK_OK);
        }
        if primary == 2_110_005 && crate::quest_destination::bichon_safe_arrival_pending(tracker) {
            line(card, format!("目的地：比奇城安全区 ({},{})", crate::quest_destination::BICHON_SAFE_X, crate::quest_destination::BICHON_SAFE_Y), FEEDBACK_OK);
            line(card, "从新手村向北前往大城；新手村安全区不算目标。", PANEL_TEXT);
        } else {
            match place(quest, tracker, entities, map, big_map) {
                Place::Current { label, .. } => line(card, label, FEEDBACK_OK),
                Place::Other(label) => line(card, format!("其他地图 · {label}"), PANEL_TEXT),
                Place::Unknown => {
                    if let Some(location) = next.and_then(|s| s.location.as_ref()) { line(card, location, PANEL_TEXT); }
                    else if quest.status == QuestStatus::InProgress {
                        line(card, "目标位置待发现 · 查看详情或地图", PANEL_TEXT);
                    }
                }
            }
        }
        button(card, "查看任务详情", QuestUiButton::SelectQuest { quest_index: primary });
        button(card, "打开大地图", QuestUiButton::OpenDestinationMap);
        if !nearby.is_empty() { line(card, "附近可顺便完成", PANEL_HIGHLIGHT); }
        for id in &nearby {
            let other = tracker.active_quests.iter().find(|q| q.quest_index == *id).unwrap();
            button(card, &format!("切换 · {} · {}", other.title, other.progress_label()), QuestUiButton::MakePrimary { quest_index: *id });
        }
        let mut remote = 0;
        let mut remaining = 0;
        for other in tracker.active_quests.iter().filter(|q| q.status.is_active() && q.quest_index != primary && !nearby.contains(&q.quest_index)) {
            if let Place::Other(label) = place(other, tracker, entities, map, big_map) {
                if remote < 2 {
                    if remote == 0 { line(card, "其他地图", PANEL_HIGHLIGHT); }
                    button(card, &format!("{} · {}", other.title, label), QuestUiButton::MakePrimary { quest_index: other.quest_index });
                    remote += 1;
                    continue;
                }
            }
            remaining += 1;
        }
        if remaining > 0 { line(card, format!("另有 {remaining} 个任务 · 按 Q 查看并设为当前"), PANEL_TEXT); }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest_model::{QuestDetailText, QuestObjective};
    fn quest(id: i32) -> Quest {
        Quest { quest_index: id, accept_npc_index: None, finish_npc_index: None, title: format!("Quest {id}"),
            npc_name: None, group: None, min_level_needed: 1, detail: QuestDetailText::default(), status: QuestStatus::InProgress,
            objectives: vec![], rewards: vec![], unknown_text: None }
    }
    #[test]
    fn manual_choice_survives_server_reordering_and_ready_state_then_falls_back() {
        let mut tracker = QuestTracker { active_quests: vec![quest(1), quest(2)] };
        let state = QuestUiState { pinned_primary_quest_index: Some(2), ..default() };
        assert_eq!(primary_quest_index(&tracker, &state, None), Some(2));
        tracker.active_quests.reverse();
        tracker.active_quests[0].status = QuestStatus::ReadyToTurnIn;
        assert_eq!(primary_quest_index(&tracker, &state, None), Some(2));
        tracker.active_quests[0].status = QuestStatus::Completed;
        assert_eq!(primary_quest_index(&tracker, &state, None), Some(1));
        assert!(state.tracked_quest_indices.is_empty());
    }
    #[test]
    fn chapter_recommendation_wins_unless_player_pins_an_active_task() {
        let tracker = QuestTracker { active_quests: vec![quest(1), quest(2)] };
        let journey = JourneyView {
            chapter_id: "test".into(), chapter_title: "Chapter".into(), level_range: "1-5".into(),
            completed_count: Some(0), quest_count: Some(2), goal: String::new(), reward_summary: String::new(),
            class_hint: None, graduated: false, graduation: None, optional: vec![],
            next: Some(crate::quest_journey::JourneyStep { quest_id: 2, title: "Second".into(), action: "Continue".into(),
                location: None, objective: None, reward: None }),
        };
        let mut state = QuestUiState::default();
        assert_eq!(primary_quest_index(&tracker, &state, Some(&journey)), Some(2));
        state.pinned_primary_quest_index = Some(1);
        assert_eq!(primary_quest_index(&tracker, &state, Some(&journey)), Some(1));
        state.pinned_primary_quest_index = Some(999);
        assert_eq!(primary_quest_index(&tracker, &state, Some(&journey)), Some(2));
    }
    #[test]
    fn hud_renders_all_counters_and_switches_only_the_chosen_primary() {
        let mut world = World::new();
        let mut q = quest(1);
        q.objectives = (0..4).map(|i| QuestObjective { objective_id: i.to_string(), text: format!("Objective {i}"), current: i, target: 10 }).collect();
        let tracker = QuestTracker { active_quests: vec![q, quest(2)] };
        let state = QuestUiState { pinned_primary_quest_index: Some(1), ..default() };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.spawn_empty().with_children(|parent| {
            assert!(render(parent, &tracker, &state, None, &EntityModelSet::default(), &MapModel::default(), None));
        });
        queue.apply(&mut world);
        let text = world.query::<&Text>().iter(&world).map(|t| t.0.as_str()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Objective 0  0/10"));
        assert!(text.contains("Objective 3  3/10"));
        assert!(text.contains("当前任务 · Quest 1"));
        assert!(!text.contains("当前任务 · Quest 2"));
    }
    #[test]
    fn full_counters_are_never_truncated() {
        let mut q = quest(1);
        q.objectives = (0..4).map(|i| QuestObjective { objective_id: i.to_string(), text: "A long objective description".repeat(5), current: i, target: 100 }).collect();
        let lines = objective_lines(&q);
        assert_eq!(lines.len(), 4);
        assert!(lines[3].ends_with("3/100"));
        assert!(lines[0].starts_with(&q.objectives[0].text));
    }
    #[test]
    fn nearest_two_other_tasks_only_use_actual_visible_targets() {
        let mut tracker = QuestTracker { active_quests: (1..=5).map(quest).collect() };
        for q in &mut tracker.active_quests { q.objectives.push(QuestObjective { objective_id: "kill".into(), text: format!("Kill Monster{}", q.quest_index), current: 0, target: 2 }); }
        let mut entities = EntityModelSet::default();
        for id in 1..=4 {
            entities.entities.push(crate::entities::EntityModel { object_id: id.to_string(), kind: crate::entities::EntityKind::Monster,
                name: format!("Monster{id}"), x: id, y: 0, level: None, direction: None });
        }
        assert_eq!(nearby_indices(1, &tracker, &entities, &MapModel::default(), None), vec![2,3]);
        assert_eq!(place(&tracker.active_quests[4], &tracker, &entities, &MapModel::default(), None), Place::Unknown);
    }
    #[test]
    fn remote_map_labels_require_known_map_and_authored_target() {
        let maps = &mir2_game_data::crystal_respawn_manifest_ref().maps;
        let bichon = maps.iter().find(|m| m.map_file_name == "0").unwrap();
        let wooma = maps.iter().find(|m| m.map_file_name == "D022").unwrap();
        assert_eq!(authored_other_map(2_110_019, Some(wooma.map_index)), None);
        assert!(authored_other_map(2_110_019, Some(bichon.map_index)).unwrap().contains(&wooma.map_title));
        assert_eq!(authored_other_map(2_110_019, None), None);
        assert_eq!(authored_other_map(12345, Some(bichon.map_index)), None);
    }

    #[test]
    fn authored_cat_region_is_nearby_without_live_entities_and_completed_or_absent_progress_is_not_inferred() {
        let bichon = mir2_game_data::crystal_map_respawns_ref("0").unwrap();
        let map = MapModel {
            center_x: 290,
            center_y: 614,
            ..default()
        };
        let big_map = BigMapModel {
            current_map_index: Some(bichon.map_index),
            ..default()
        };
        let mut cats = quest(2_110_003);
        cats.objectives = vec![
            QuestObjective {
                objective_id: "2110003:0".into(),
                text: "Scarecrow".into(),
                current: 2,
                target: 2,
            },
            QuestObjective {
                objective_id: "2110003:1".into(),
                text: "RakingCat".into(),
                current: 0,
                target: 2,
            },
        ];
        let tracker = QuestTracker {
            active_quests: vec![quest(1), cats.clone()],
        };
        let empty_entities = EntityModelSet::default();
        assert_eq!(
            place(&cats, &tracker, &empty_entities, &map, Some(&big_map)),
            Place::Current {
                label: "狩猎区域 · RakingCat (340,550) · 64 格".into(),
                distance: 64,
            },
        );
        assert_eq!(
            nearby_indices(1, &tracker, &empty_entities, &map, Some(&big_map)),
            vec![2_110_003],
        );

        cats.objectives[1].current = 2;
        assert_eq!(
            place(&cats, &tracker, &empty_entities, &map, Some(&big_map)),
            Place::Unknown,
            "completed objectives must not create an authored target",
        );
        cats.objectives.clear();
        assert_eq!(
            place(&cats, &tracker, &empty_entities, &map, Some(&big_map)),
            Place::Unknown,
            "missing progress never infers a hunt objective",
        );
    }
}
