//! Local task prioritization; all objectives and destinations remain read-only.
use super::*;
use crate::big_map::BigMapModel;
use crate::quest_model::QuestStatus;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

// The tracker is 304 px wide, with 282 px of usable content (270 px inside a
// button). Count wide Latin glyphs as two columns alongside CJK glyphs; this
// leaves margin at 12 px Microsoft YaHei. Bevy's intrinsic text height can lag
// a wrapped line by a frame, so every row reserves its full height up front.
const CARD_WRAP_COLUMNS: usize = 38;
const CARD_LINE_HEIGHT: f32 = 18.0;

#[derive(Component)]
pub(super) struct GuidanceViewport;

fn clamped_scroll(current: f32, delta: f32, maximum: f32) -> f32 {
    (current - delta).clamp(0.0, maximum.max(0.0))
}

pub(super) fn scroll_tracker(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut state: ResMut<QuestUiState>,
    shell: Option<Res<NativeShellModel>>,
    ui: Res<NativePlayerUiState>,
    mut viewports: Query<
        (&RelativeCursorPosition, &ComputedNode, &mut ScrollPosition),
        With<GuidanceViewport>,
    >,
) {
    let delta: f32 = wheel
        .read()
        .map(|event| match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * CARD_LINE_HEIGHT * 3.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y,
        })
        .sum();
    if delta == 0.0
        || shell.is_none_or(|shell| shell.screen != NativeShellScreen::InGame)
        || ui.menu_pointer_consumed
        || ui.amount_modal_open()
    {
        return;
    }
    for (cursor, computed, mut position) in &mut viewports {
        if !cursor.cursor_over() || computed.size().y <= 0.0 {
            continue;
        }
        let maximum =
            (computed.content_size().y - computed.size().y) * computed.inverse_scale_factor;
        let next = clamped_scroll(position.y, delta, maximum);
        if next != state.guidance_scroll_y {
            state.guidance_scroll_y = next;
            position.y = next;
        }
    }
}

/// Manual active choice wins until authoritative completion/removal. Journey's
/// recommended next step (including accept/turn-in) otherwise remains primary.
pub fn primary_quest_index(
    tracker: &QuestTracker,
    state: &QuestUiState,
    journey: Option<&JourneyView>,
) -> Option<i32> {
    state
        .pinned_primary_quest_index
        .filter(|id| {
            tracker
                .active_quests
                .iter()
                .any(|q| q.quest_index == *id && q.status.is_active())
        })
        .or_else(|| {
            journey
                .and_then(|j| j.next.as_ref())
                .map(|step| step.quest_id)
                .filter(|id| {
                    tracker
                        .active_quests
                        .iter()
                        .any(|q| q.quest_index == *id && !q.status.is_finished())
                })
        })
        .or_else(|| {
            visible_tracker_quests(tracker, state)
                .first()
                .map(|q| q.quest_index)
        })
        .or_else(|| {
            tracker
                .active_quests
                .iter()
                .find(|q| q.status.is_active())
                .map(|q| q.quest_index)
        })
}

#[derive(Debug, PartialEq, Eq)]
enum Place {
    Current { label: String, distance: u32 },
    Route(crate::quest_ui::route::QuestRouteStep),
    Other(String),
    Unknown,
}

fn place(
    quest: &Quest,
    tracker: &QuestTracker,
    entities: &EntityModelSet,
    map: &MapModel,
    big_map: Option<&BigMapModel>,
) -> Place {
    if quest.status == QuestStatus::InProgress {
        let targets = crate::quest_destination::active_target_map_indices(quest);
        if !targets.is_empty() {
            let Some(current_map) = big_map.and_then(|model| model.current_map_index) else {
                return Place::Unknown;
            };
            // A nearby name cannot replace an authored destination on another
            // map. Keep the entrance action until authoritative map arrival.
            if !targets.contains(&current_map) {
                if let crate::quest_ui::route::QuestRoute::NextStep(step) =
                    crate::quest_ui::route::resolve(Some(current_map), &targets)
                {
                    return Place::Route(step);
                }
                return authored_other_map(quest, Some(current_map))
                    .map_or(Place::Unknown, Place::Other);
            }
        }
    }
    if let Some(target) = nearest_quest_monster(
        tracker,
        Some(quest.quest_index),
        entities,
        map.center_x,
        map.center_y,
    ) {
        return Place::Current {
            label: format!(
                "{} ({},{}) · {} 格",
                crate::player_text::name(&target.entity.name),
                target.entity.x,
                target.entity.y,
                target.distance
            ),
            distance: target.distance,
        };
    }
    if let Some(npc) =
        super::turn_in::destination_on_map(quest, big_map.and_then(|map| map.current_map_index))
    {
        return if big_map.and_then(|model| model.current_map_index) == Some(npc.map_index) {
            Place::Current {
                label: npc.label(),
                distance: map
                    .center_x
                    .abs_diff(npc.x)
                    .max(map.center_y.abs_diff(npc.y)),
            }
        } else if mir2_game_data::periodic_quests::is_periodic(quest.quest_index) {
            match super::route::resolve(
                big_map.and_then(|map| map.current_map_index),
                &[npc.map_index],
            ) {
                super::route::QuestRoute::NextStep(step) => Place::Route(step),
                _ => Place::Other(npc.label()),
            }
        } else {
            Place::Other(npc.label())
        };
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
                        crate::player_text::name(&region.name),
                        region.center.x,
                        region.center.y,
                        distance
                    ),
                    distance,
                };
            }
        }
    }
    Place::Unknown
}

fn authored_other_map(quest: &Quest, current_map: Option<i32>) -> Option<String> {
    let current_map = current_map?;
    let targets = crate::quest_destination::active_target_map_indices(quest);
    if targets.contains(&current_map) {
        return None;
    }
    let maps = &mir2_game_data::crystal_respawn_manifest_ref().maps;
    let labels = targets
        .into_iter()
        .filter_map(|target| {
            maps.iter()
                .find(|map| map.map_index == target)
                .map(|map| crate::player_text::name(&map.map_title))
        })
        .collect::<Vec<_>>();
    (!labels.is_empty()).then(|| labels.join(" / "))
}

fn hunt_navigation(
    quest: &Quest,
    map: &MapModel,
    big_map: Option<&BigMapModel>,
) -> Option<QuestRouteNavigationIntent> {
    let big_map = big_map?;
    let map_index = big_map.current_map_index?;
    let region = crate::quest_hunt_regions::active_hunt_regions(
        &QuestTracker {
            active_quests: vec![quest.clone()],
        },
        map_index,
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
    })?;
    Some(QuestRouteNavigationIntent {
        target: QuestRouteTarget::HuntRegion {
            monster_index: region.monster_index,
            radius: region.radius,
        },
        quest_index: quest.quest_index,
        reset_epoch: big_map.reset_epoch,
        map_index,
        x: region.center.x,
        y: region.center.y,
    })
}

fn task_npc_navigation(
    quest: &Quest,
    big_map: Option<&BigMapModel>,
) -> Option<QuestRouteNavigationIntent> {
    if !mir2_game_data::periodic_quests::is_periodic(quest.quest_index) {
        return None;
    }
    let big_map = big_map?;
    let target = super::turn_in::destination_on_map(quest, big_map.current_map_index)?;
    if big_map.current_map_index != Some(target.map_index) {
        return None;
    }
    Some(QuestRouteNavigationIntent {
        target: QuestRouteTarget::TaskNpc {
            object_id: target.object_id,
        },
        quest_index: quest.quest_index,
        reset_epoch: big_map.reset_epoch,
        map_index: target.map_index,
        x: target.x,
        y: target.y,
    })
}

fn nearby_indices(
    primary: i32,
    tracker: &QuestTracker,
    entities: &EntityModelSet,
    map: &MapModel,
    big_map: Option<&BigMapModel>,
) -> Vec<i32> {
    let mut candidates = tracker
        .active_quests
        .iter()
        .filter(|q| q.quest_index != primary && q.status.is_active())
        .filter_map(|q| match place(q, tracker, entities, map, big_map) {
            Place::Current { distance, .. } => Some((distance, q.quest_index)),
            _ => None,
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.into_iter().take(2).map(|(_, id)| id).collect()
}

fn objective_lines(quest: &Quest, class_name: &str) -> Vec<String> {
    let practice = crate::quest_practice::practice_guide(quest.quest_index, class_name);
    quest
        .objectives
        .iter()
        .enumerate()
        .map(|(index, o)| {
            if let Some(guide) = practice
                .as_ref()
                .filter(|guide| guide.objective_index == index)
            {
                if o.target > 0 && o.current >= o.target {
                    crate::player_text::format_named(
                        "quest.practice.completed",
                        "职业练习已完成  {current}/{total}",
                        &[
                            ("current", &o.current.to_string()),
                            ("total", &o.target.to_string()),
                        ],
                    )
                } else {
                    crate::player_text::format_named(
                        "quest.practice.progress",
                        "职业练习 {current}/{total}：{summary}",
                        &[
                            ("current", &o.current.to_string()),
                            ("total", &o.target.to_string()),
                            ("summary", &guide.summary),
                        ],
                    )
                }
            } else {
                format!(
                    "{}  {}/{}",
                    crate::player_text::quest_objective_label(quest.quest_index, index, &o.text),
                    o.current,
                    o.target
                )
            }
        })
        .collect()
}

fn card_char_columns(ch: char) -> usize {
    let width = UnicodeWidthChar::width(ch).unwrap_or(0);
    if ch.is_ascii_uppercase() || matches!(ch, 'm' | 'w' | '@' | '#' | '%' | '&') {
        width.max(2)
    } else {
        width
    }
}

fn card_columns(text: &str) -> usize {
    text.chars().map(card_char_columns).sum()
}

pub(super) fn wrap_card_text(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if card_columns(paragraph) <= CARD_WRAP_COLUMNS {
            lines.push(paragraph.to_owned());
            continue;
        }
        let mut current = String::new();
        let mut width = 0;
        for word in paragraph.split_whitespace() {
            let word_width = card_columns(word);
            if !current.is_empty() && width + 1 + word_width > CARD_WRAP_COLUMNS {
                lines.push(std::mem::take(&mut current));
                width = 0;
            }
            if word_width > CARD_WRAP_COLUMNS {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    width = 0;
                }
                for grapheme in word.graphemes(true) {
                    let char_width = card_columns(grapheme);
                    if !current.is_empty() && width + char_width > CARD_WRAP_COLUMNS {
                        lines.push(std::mem::take(&mut current));
                        width = 0;
                    }
                    current.push_str(grapheme);
                    width += char_width;
                }
            } else {
                if !current.is_empty() {
                    current.push(' ');
                    width += 1;
                }
                current.push_str(word);
                width += word_width;
            }
        }
        lines.push(current);
    }
    lines
}

fn card_text_height(text: &str) -> f32 {
    wrap_card_text(text).len() as f32 * CARD_LINE_HEIGHT
}

fn line(parent: &mut ChildSpawnerCommands, text: impl Into<String>, color: Color) {
    let wrapped = wrap_card_text(&crate::player_text::text(&text.into()));
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(wrapped.len() as f32 * CARD_LINE_HEIGHT),
            flex_shrink: 0.0,
            ..default()
        },
        Text::new(wrapped.join("\n")),
        crate::crystal_ui::typography::crystal_text_font(12.0),
        TextColor(color),
        TextLayout::new(Justify::Start, LineBreak::NoWrap),
    ));
}

fn button(parent: &mut ChildSpawnerCommands, text: &str, action: QuestUiButton) {
    let text = crate::player_text::text(text);
    let height = (card_text_height(&text) + 6.0).max(25.0);
    parent
        .spawn((
            Button,
            action,
            QuestUiButtonVisual { enabled: true },
            Node {
                height: Val::Px(height),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            FocusPolicy::Block,
        ))
        .with_children(|node| line(node, text, PANEL_HIGHLIGHT));
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    state: &QuestUiState,
    journey: Option<&JourneyView>,
    entities: &EntityModelSet,
    map: &MapModel,
    big_map: Option<&BigMapModel>,
    class_name: &str,
    supplies: &crate::quest_supplies::SupplyPlan,
) -> bool {
    let Some(primary) = primary_quest_index(tracker, state, journey) else {
        return false;
    };
    let Some(quest) = tracker
        .active_quests
        .iter()
        .find(|q| q.quest_index == primary)
    else {
        return false;
    };
    let periodic = mir2_game_data::periodic_quests::quest(quest.quest_index);
    let nearby = nearby_indices(primary, tracker, entities, map, big_map);
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(16.0),
                width: Val::Px(304.0),
                max_height: Val::Percent(95.0),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.03, 0.02, 0.90)),
            BorderColor::all(PANEL_HIGHLIGHT),
            GuidanceViewport,
            RelativeCursorPosition::default(),
            ScrollPosition(Vec2::new(0.0, state.guidance_scroll_y)),
            FocusPolicy::Block,
        ))
        .with_children(|card| {
            if let Some(journey) = journey.filter(|_| periodic.is_none()) {
                line(
                    card,
                    format!(
                        "{} · {}",
                        crate::player_text::text(&journey.chapter_title),
                        crate::player_text::text(&journey.progress_label())
                    ),
                    PANEL_TEXT,
                );
            }
            if let Some(definition) = periodic {
                line(
                    card,
                    match definition.cadence {
                        mir2_game_data::periodic_quests::PeriodicCadence::Daily => "Daily Tasks",
                        mir2_game_data::periodic_quests::PeriodicCadence::Weekly => "Weekly Tasks",
                    },
                    PANEL_HIGHLIGHT,
                );
            }
            line(
                card,
                format!(
                    "当前任务 · {}",
                    crate::player_text::quest_title(quest.quest_index, &quest.title)
                ),
                PANEL_HIGHLIGHT,
            );
            for objective in objective_lines(quest, class_name) {
                line(card, objective, Color::WHITE);
            }
            let next = journey
                .and_then(|j| j.next.as_ref())
                .filter(|s| s.quest_id == primary);
            if quest.status == QuestStatus::ReadyToTurnIn {
                let action = if periodic.is_some() {
                    "Return to either town Task Steward.".into()
                } else {
                    next.map(|s| s.action.clone()).unwrap_or_else(|| {
                        quest
                            .npc_name
                            .as_ref()
                            .map(|n| format!("返回 {} 交付任务", crate::player_text::name(n)))
                            .unwrap_or_else(|| "打开任务详情查看交付方式".into())
                    })
                };
                line(card, action, FEEDBACK_OK);
            } else if quest.status == QuestStatus::NotStarted {
                line(
                    card,
                    if periodic.is_some() {
                        "Visit a Task Steward in Bichon or Mongchon to view available tasks."
                    } else {
                        next.map(|s| s.action.as_str())
                            .unwrap_or("查看详情领取任务")
                    },
                    FEEDBACK_OK,
                );
            }
            let route = if primary == 2_110_005
                && crate::quest_destination::bichon_safe_arrival_pending(tracker)
            {
                line(
                    card,
                    format!(
                        "目的地：比奇城安全区 ({},{})",
                        crate::quest_destination::BICHON_SAFE_X,
                        crate::quest_destination::BICHON_SAFE_Y
                    ),
                    FEEDBACK_OK,
                );
                line(
                    card,
                    "从新手村向北前往大城；新手村安全区不算目标。",
                    PANEL_TEXT,
                );
                None
            } else {
                match place(quest, tracker, entities, map, big_map) {
                    Place::Current { label, .. } => {
                        line(card, label, FEEDBACK_OK);
                        None
                    }
                    Place::Route(step) => {
                        line(
                            card,
                            format!(
                                "当前地图 · {} · 当前位置 ({},{})",
                                crate::player_text::name(&step.current_map_title),
                                map.center_x,
                                map.center_y
                            ),
                            PANEL_TEXT,
                        );
                        line(
                            card,
                            format!(
                                "下一步 · 入口 ({},{}) · 进入 {}",
                                step.entrance_x,
                                step.entrance_y,
                                crate::player_text::name(&step.next_map_title)
                            ),
                            FEEDBACK_OK,
                        );
                        if step.remaining_hops > 1 {
                            line(
                                card,
                                format!("到目标仍需经过 {} 个地图入口", step.remaining_hops),
                                PANEL_TEXT,
                            );
                        }
                        Some(step)
                    }
                    Place::Other(label) => {
                        line(card, format!("其他地图 · {label}"), PANEL_TEXT);
                        None
                    }
                    Place::Unknown => {
                        if let Some(location) = next.and_then(|s| s.location.as_ref()) {
                            line(card, location, PANEL_TEXT);
                        } else if quest.status == QuestStatus::InProgress {
                            line(card, "目标位置待发现 · 查看详情或地图", PANEL_TEXT);
                        }
                        None
                    }
                }
            };
            if let (Some(step), Some(big_map)) = (route, big_map) {
                button(
                    card,
                    "前往入口 · 自动寻路",
                    QuestUiButton::NavigateQuestRoute(
                        crate::quest_ui::QuestRouteNavigationIntent {
                            target: QuestRouteTarget::Entrance,
                            quest_index: primary,
                            reset_epoch: big_map.reset_epoch,
                            map_index: step.current_map_index,
                            x: step.entrance_x,
                            y: step.entrance_y,
                        },
                    ),
                );
            } else if let Some(intent) = task_npc_navigation(quest, big_map) {
                button(
                    card,
                    "Go to Task Steward · Auto-route",
                    QuestUiButton::NavigateQuestRoute(intent),
                );
            } else if let Some(intent) = hunt_navigation(quest, map, big_map) {
                // Keep this action independent of live monster visibility. A
                // nearby target hint must not hide the authored hunting area.
                button(
                    card,
                    "前往狩猎区域 · 自动寻路",
                    QuestUiButton::NavigateQuestRoute(intent),
                );
            }
            if let Some(feedback) = state.feedback.as_ref() {
                line(
                    card,
                    feedback.message.clone(),
                    if feedback.is_error {
                        FEEDBACK_ERR
                    } else {
                        FEEDBACK_OK
                    },
                );
            }
            button(
                card,
                "查看任务详情",
                QuestUiButton::SelectQuest {
                    quest_index: primary,
                },
            );
            button(card, "打开大地图", QuestUiButton::OpenDestinationMap);
            button(card, &supplies.summary(), QuestUiButton::ToggleSupplies);
            if !nearby.is_empty() {
                line(card, "附近可顺便完成", PANEL_HIGHLIGHT);
            }
            for id in &nearby {
                let other = tracker
                    .active_quests
                    .iter()
                    .find(|q| q.quest_index == *id)
                    .unwrap();
                button(
                    card,
                    &format!(
                        "切换 · {} · {}",
                        crate::player_text::quest_title(other.quest_index, &other.title),
                        other.progress_label()
                    ),
                    QuestUiButton::MakePrimary { quest_index: *id },
                );
            }
            let mut remote = 0;
            let mut remaining = 0;
            for other in tracker.active_quests.iter().filter(|q| {
                q.status.is_active() && q.quest_index != primary && !nearby.contains(&q.quest_index)
            }) {
                if let Place::Other(label) = place(other, tracker, entities, map, big_map) {
                    if remote < 2 {
                        if remote == 0 {
                            line(card, "其他地图", PANEL_HIGHLIGHT);
                        }
                        button(
                            card,
                            &format!(
                                "{} · {}",
                                crate::player_text::quest_title(other.quest_index, &other.title),
                                label
                            ),
                            QuestUiButton::MakePrimary {
                                quest_index: other.quest_index,
                            },
                        );
                        remote += 1;
                        continue;
                    }
                }
                remaining += 1;
            }
            if remaining > 0 {
                line(
                    card,
                    format!("另有 {remaining} 个任务 · 按 Q 查看并设为当前"),
                    PANEL_TEXT,
                );
            }
        });
    true
}

/// Supplies replace the tracker while expanded, so the six inventory rows and
/// route guidance cannot push quest text into the bottom HUD.
pub(super) fn render_supplies(
    parent: &mut ChildSpawnerCommands,
    state: &QuestUiState,
    big_map: Option<&BigMapModel>,
    supplies: &crate::quest_supplies::SupplyPlan,
) {
    let displayed_cost = supplies
        .rows
        .iter()
        .filter(|row| {
            state
                .supply_vendor
                .is_none_or(|vendor| row.vendor == vendor)
        })
        .map(|row| row.shortage().saturating_mul(row.unit_price))
        .fold(0_u32, u32::saturating_add);
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(16.0),
                width: Val::Px(304.0),
                max_height: Val::Percent(95.0),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.03, 0.02, 0.96)),
            BorderColor::all(PANEL_HIGHLIGHT),
            GuidanceViewport,
            RelativeCursorPosition::default(),
            ScrollPosition(Vec2::new(0.0, state.guidance_scroll_y)),
            FocusPolicy::Block,
        ))
        .with_children(|card| {
            line(card, "出发补给 · 现有 / 建议", PANEL_HIGHLIGHT);
            for row in supplies.rows.iter().filter(|row| {
                state
                    .supply_vendor
                    .is_none_or(|vendor| row.vendor == vendor)
            }) {
                line(
                    card,
                    row.line(),
                    if row.is_low() {
                        FEEDBACK_ERR
                    } else {
                        PANEL_TEXT
                    },
                );
            }
            if state
                .supply_vendor
                .is_none_or(|vendor| vendor == crate::quest_supplies::SupplyVendor::Potions)
            {
                let medicines = if supplies.rows.iter().any(|row| row.label == "蓝药") {
                    "红药、蓝药"
                } else {
                    "红药"
                };
                line(
                    card,
                    crate::player_text::format_named(
                        "quest.supply.recommended",
                        "当前推荐：{size}{medicines}",
                        &[
                            (
                                "size",
                                &crate::player_text::supply_size(supplies.potion_size),
                            ),
                            ("medicines", &crate::player_text::text(medicines)),
                        ],
                    ),
                    PANEL_TEXT,
                );
            }
            line(
                card,
                format!("金币 {} · 补足约 {}", supplies.gold, displayed_cost),
                if supplies.gold < displayed_cost {
                    FEEDBACK_ERR
                } else {
                    PANEL_TEXT
                },
            );
            if supplies.gold < displayed_cost {
                line(card, "金币不足：优先急缺品和回城卷。", FEEDBACK_ERR);
            }
            if let Some((weight, max)) = supplies.bag_weight {
                line(
                    card,
                    format!("背包负重 {weight}/{max}"),
                    if weight >= max {
                        FEEDBACK_ERR
                    } else {
                        PANEL_TEXT
                    },
                );
            }
            if supplies.bag_full {
                line(card, "背包已满，可先出售闲置物品。", FEEDBACK_ERR);
            }
            if supplies.needs_taoist_material_guidance {
                line(card, "火符/召唤装备符；施毒可用携带毒粉。", PANEL_TEXT);
            }
            for vendor in crate::quest_supplies::SupplyVendor::ALL {
                if state.supply_vendor.is_some() {
                    break;
                }
                if vendor == crate::quest_supplies::SupplyVendor::Poison
                    && !supplies.rows.iter().any(|row| row.vendor == vendor)
                {
                    continue;
                }
                button(
                    card,
                    &format!("购买 · {}", crate::player_text::text(vendor.goods())),
                    QuestUiButton::SelectSupplyVendor(vendor),
                );
            }
            if let Some(vendor) = state.supply_vendor {
                if let Some(destination) = vendor.destination() {
                    line(
                        card,
                        format!(
                            "{} ({},{})",
                            crate::player_text::name(vendor.npc_name()),
                            destination.x,
                            destination.y
                        ),
                        PANEL_HIGHLIGHT,
                    );
                }
                if let Some((big_map, route)) = big_map.and_then(|model| {
                    vendor
                        .route(model.current_map_index?)
                        .map(|route| (model, route))
                }) {
                    if let Some(next) = &route.next_map_title {
                        line(
                            card,
                            format!(
                                "下一段：{}入口 ({},{})",
                                crate::player_text::name(next),
                                route.x,
                                route.y
                            ),
                            PANEL_TEXT,
                        );
                    } else {
                        line(card, "到店后点击商人购买，以店内报价为准。", PANEL_TEXT);
                    }
                    button(
                        card,
                        if route.is_entrance {
                            "前往店铺入口 · 自动寻路"
                        } else {
                            "前往商人 · 自动寻路"
                        },
                        QuestUiButton::NavigateQuestRoute(QuestRouteNavigationIntent {
                            target: QuestRouteTarget::Supply { vendor },
                            quest_index: 0,
                            reset_epoch: big_map.reset_epoch,
                            map_index: route.map_index,
                            x: route.x,
                            y: route.y,
                        }),
                    );
                } else {
                    line(card, "暂无可走路线，请打开大地图查看入口。", FEEDBACK_ERR);
                }
            }
            // Route feedback is bounded to a single wrapped message and refreshes
            // through the existing epoch/ordinary movement state machine.
            if let Some(feedback) = state.feedback.as_ref() {
                line(
                    card,
                    &feedback.message,
                    if feedback.is_error {
                        FEEDBACK_ERR
                    } else {
                        FEEDBACK_OK
                    },
                );
            }
            if state.supply_vendor.is_some() {
                button(card, "查看全部补给", QuestUiButton::ShowSupplyInventory);
            }
            button(card, "返回任务引导", QuestUiButton::ToggleSupplies);
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_supply_vendor_names_match_the_routed_npc_in_every_locale() {
        use crate::{
            native_i18n::{self, Locale},
            quest_supplies::SupplyVendor,
        };
        for locale in Locale::ALL {
            native_i18n::with_locale(locale, || {
                for vendor in SupplyVendor::ALL {
                    let destination = vendor.destination().unwrap();
                    let big_map = BigMapModel {
                        current_map_index: Some(destination.map_index),
                        ..default()
                    };
                    let translated = crate::player_text::name(vendor.npc_name());
                    assert_ne!(
                        translated,
                        vendor.npc_name(),
                        "catalog must cover the actual NPC"
                    );
                    if locale == Locale::TraditionalChinese {
                        assert_eq!(
                            translated,
                            match vendor {
                                SupplyVendor::Potions => "鍊金師 Samuel",
                                SupplyVendor::General => "商人 Bull",
                                SupplyVendor::Poison => "專家 Travis",
                            }
                        );
                    }
                    let mut world = World::new();
                    let mut queue = bevy::ecs::world::CommandQueue::default();
                    Commands::new(&mut queue, &world)
                        .spawn_empty()
                        .with_children(|parent| {
                            render_supplies(
                                parent,
                                &QuestUiState {
                                    supply_open: true,
                                    supply_vendor: Some(vendor),
                                    ..default()
                                },
                                Some(&big_map),
                                &test_supplies(),
                            );
                        });
                    queue.apply(&mut world);
                    let expected = format!("{} ({},{})", translated, destination.x, destination.y);
                    let compact = |text: &str| {
                        text.chars()
                            .filter(|ch| !ch.is_whitespace())
                            .collect::<String>()
                    };
                    assert!(world
                        .query::<&Text>()
                        .iter(&world)
                        .any(|text| compact(&text.0) == compact(&expected)));
                    assert!(world.query::<&QuestUiButton>().iter(&world).any(|button| match button {
                        QuestUiButton::NavigateQuestRoute(intent) => {
                            matches!(intent.target, QuestRouteTarget::Supply { vendor: actual } if actual == vendor)
                                && (intent.map_index, intent.x, intent.y) == (destination.map_index, destination.x, destination.y)
                        }
                        _ => false,
                    }), "localized display must keep the original route action");
                }
            });
        }
    }

    #[test]
    fn native_portuguese_practice_wraps_before_height_and_keeps_controls_reachable() {
        crate::native_i18n::with_locale(crate::native_i18n::Locale::BrazilianPortuguese, || {
            let mut current = quest(2_110_021);
            current.objectives = vec![
                QuestObjective {
                    objective_id: "kill".into(),
                    text: "Defeat 3 WoomaFighter.".into(),
                    current: 1,
                    target: 3,
                },
                QuestObjective {
                    objective_id: "practice".into(),
                    text: "Complete your class practice".into(),
                    current: 0,
                    target: 1,
                },
            ];
            let lines = objective_lines(&current, "Taoist");
            assert!(lines[1].starts_with("Treino de classe 0/1:"));
            assert!(lines[1].contains("Talismã de Fogo"));
            for line in &lines {
                let wrapped = wrap_card_text(line);
                assert!(wrapped
                    .iter()
                    .all(|row| card_columns(row) <= CARD_WRAP_COLUMNS));
                assert_eq!(
                    card_text_height(line),
                    wrapped.len() as f32 * CARD_LINE_HEIGHT
                );
            }
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let state = QuestUiState {
                guidance_scroll_y: 90.0,
                ..default()
            };
            Commands::new(&mut queue, &world)
                .spawn_empty()
                .with_children(|parent| {
                    render_supplies(parent, &state, None, &test_supplies());
                });
            queue.apply(&mut world);
            let (node, scroll) = world
                .query_filtered::<(&Node, &ScrollPosition), With<GuidanceViewport>>()
                .single(&world)
                .unwrap();
            assert_eq!(node.max_height, Val::Percent(95.0));
            assert_eq!(node.overflow, Overflow::scroll_y());
            assert_eq!(scroll.y, 90.0);
            assert!(world
                .query::<&QuestUiButton>()
                .iter(&world)
                .any(|button| matches!(button, QuestUiButton::ToggleSupplies)));
        });
    }

    #[test]
    fn native_tracker_wheel_only_scrolls_hovered_viewport_and_clamps_to_content() {
        use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
        let mut app = App::new();
        app.add_message::<MouseWheel>()
            .init_resource::<QuestUiState>()
            .init_resource::<NativePlayerUiState>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        let viewport = app
            .world_mut()
            .spawn((
                GuidanceViewport,
                RelativeCursorPosition {
                    cursor_over: false,
                    ..default()
                },
                ComputedNode {
                    size: Vec2::new(304.0, 300.0),
                    content_size: Vec2::new(304.0, 400.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .id();
        app.add_systems(Update, scroll_tracker);
        let wheel = MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -3.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        };
        app.world_mut().write_message(wheel.clone());
        app.update();
        assert_eq!(
            app.world().resource::<QuestUiState>().guidance_scroll_y,
            0.0
        );
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(viewport)
            .unwrap()
            .cursor_over = true;
        app.world_mut().write_message(wheel);
        app.update();
        assert_eq!(
            app.world().resource::<QuestUiState>().guidance_scroll_y,
            100.0
        );
        assert_eq!(
            app.world().get::<ScrollPosition>(viewport).unwrap().y,
            100.0
        );
    }

    fn test_supplies() -> crate::quest_supplies::SupplyPlan {
        crate::quest_supplies::plan(
            &crate::read_model::PlayerStats::default(),
            &crate::inventory::InventoryModel::default(),
            None,
            None,
        )
    }

    use super::*;
    use crate::quest_model::{QuestDetailText, QuestObjective};
    fn quest(id: i32) -> Quest {
        Quest {
            quest_index: id,
            accept_npc_index: None,
            finish_npc_index: None,
            title: format!("Quest {id}"),
            npc_name: None,
            group: None,
            min_level_needed: 1,
            detail: QuestDetailText::default(),
            status: QuestStatus::InProgress,
            objectives: vec![],
            rewards: vec![],
            unknown_text: None,
        }
    }
    #[test]
    fn manual_choice_survives_server_reordering_and_ready_state_then_falls_back() {
        let mut tracker = QuestTracker {
            active_quests: vec![quest(1), quest(2)],
        };
        let state = QuestUiState {
            pinned_primary_quest_index: Some(2),
            ..default()
        };
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
        let tracker = QuestTracker {
            active_quests: vec![quest(1), quest(2)],
        };
        let journey = JourneyView {
            chapter_id: "test".into(),
            chapter_title: "Chapter".into(),
            level_range: "1-5".into(),
            completed_count: Some(0),
            quest_count: Some(2),
            goal: String::new(),
            reward_summary: String::new(),
            class_hint: None,
            graduated: false,
            graduation: None,
            optional: vec![],
            next: Some(crate::quest_journey::JourneyStep {
                quest_id: 2,
                title: "Second".into(),
                action: "Continue".into(),
                location: None,
                objective: None,
                reward: None,
            }),
        };
        let mut state = QuestUiState::default();
        assert_eq!(
            primary_quest_index(&tracker, &state, Some(&journey)),
            Some(2)
        );
        state.pinned_primary_quest_index = Some(1);
        assert_eq!(
            primary_quest_index(&tracker, &state, Some(&journey)),
            Some(1)
        );
        state.pinned_primary_quest_index = Some(999);
        assert_eq!(
            primary_quest_index(&tracker, &state, Some(&journey)),
            Some(2)
        );
    }
    #[test]
    fn hud_renders_all_counters_and_switches_only_the_chosen_primary() {
        let mut world = World::new();
        let mut q = quest(1);
        q.objectives = (0..4)
            .map(|i| QuestObjective {
                objective_id: i.to_string(),
                text: format!("Objective {i}"),
                current: i,
                target: 10,
            })
            .collect();
        let tracker = QuestTracker {
            active_quests: vec![q, quest(2)],
        };
        let state = QuestUiState {
            pinned_primary_quest_index: Some(1),
            ..default()
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.spawn_empty().with_children(|parent| {
            assert!(render(
                parent,
                &tracker,
                &state,
                None,
                &EntityModelSet::default(),
                &MapModel::default(),
                None,
                "Warrior",
                &test_supplies()
            ));
        });
        queue.apply(&mut world);
        let text = world
            .query::<&Text>()
            .iter(&world)
            .map(|t| t.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Objective 0  0/10"));
        assert!(text.contains("Objective 3  3/10"));
        assert!(text.contains("当前任务 · Quest 1"));
        assert!(!text.contains("当前任务 · Quest 2"));
    }
    #[test]
    fn practice_card_names_skills_for_the_players_class_and_keeps_server_completion() {
        let mut q = quest(2110021);
        q.objectives = vec![
            QuestObjective {
                objective_id: "kill".into(),
                text: "Defeat 3 WoomaFighter".into(),
                current: 3,
                target: 3,
            },
            QuestObjective {
                objective_id: "practice".into(),
                text: "Complete your class practice".into(),
                current: 0,
                target: 1,
            },
        ];
        let lines = objective_lines(&q, "Warrior");
        assert!(lines[1].contains("半月弯刀造成伤害；刺杀剑术造成伤害"));
        assert!(lines[1].contains("0/1"));
        assert!(wrap_card_text(&lines[1])
            .iter()
            .all(|line| card_columns(line) <= CARD_WRAP_COLUMNS));
        assert!(objective_lines(&q, "Wizard")[1].contains("雷电术"));
        q.objectives[1].current = 1;
        assert_eq!(objective_lines(&q, "Warrior")[1], "职业练习已完成  1/1");
    }
    #[test]
    fn full_counters_are_never_truncated() {
        let mut q = quest(1);
        q.objectives = (0..4)
            .map(|i| QuestObjective {
                objective_id: i.to_string(),
                text: "A long objective description".repeat(5),
                current: i,
                target: 100,
            })
            .collect();
        let lines = objective_lines(&q, "Warrior");
        assert_eq!(lines.len(), 4);
        assert!(lines[3].ends_with("3/100"));
        assert!(lines[0].starts_with(&q.objectives[0].text));
    }

    #[test]
    fn long_route_and_other_map_rows_reserve_every_wrapped_line() {
        let route = "下一步 · 入口 (84,277) · 进入 WoomaTempleEntrance";
        let other = "Help Needed · SerpentValley · Merchant Robert (505,479)";
        for text in [route, other, "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"] {
            let wrapped = wrap_card_text(text);
            assert!(wrapped.len() > 1, "{text}");
            assert!(
                wrapped
                    .iter()
                    .all(|line| card_columns(line) <= CARD_WRAP_COLUMNS),
                "{wrapped:?}"
            );
            assert_eq!(
                wrapped.join(" ").split_whitespace().collect::<String>(),
                text.split_whitespace().collect::<String>()
            );
        }

        let mut world = World::new();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.spawn_empty().with_children(|parent| {
            line(parent, route, FEEDBACK_OK);
            button(parent, other, QuestUiButton::SelectQuest { quest_index: 1 });
        });
        queue.apply(&mut world);

        let route_row = world
            .query::<(&Text, &Node, &TextLayout)>()
            .iter(&world)
            .find(|(text, _, _)| text.0.contains("WoomaTempleEntrance"))
            .expect("route row");
        assert_eq!(route_row.1.height, Val::Px(card_text_height(route)));
        assert_eq!(route_row.2.linebreak, LineBreak::NoWrap);
        let button_row = world
            .query::<(&QuestUiButton, &Node)>()
            .iter(&world)
            .next()
            .expect("other-map button");
        assert_eq!(
            button_row.1.height,
            Val::Px((card_text_height(other) + 6.0).max(25.0))
        );
    }

    #[test]
    fn forced_line_breaks_preserve_indic_thai_and_combining_graphemes() {
        for word in [
            "क्षि".repeat(42),
            "น้ำ".repeat(42),
            "a\u{0301}".repeat(80),
            "اللّغة".repeat(20),
        ] {
            let lines = wrap_card_text(&word);
            assert!(lines.len() > 1);
            assert_eq!(lines.concat(), word);
            let valid_boundaries = word
                .grapheme_indices(true)
                .map(|(index, _)| index)
                .chain(std::iter::once(word.len()))
                .collect::<std::collections::HashSet<_>>();
            let mut end = 0;
            for line in &lines {
                end += line.len();
                assert!(
                    valid_boundaries.contains(&end),
                    "split a shaping cluster in {word:?}"
                );
            }
        }
    }

    #[test]
    fn long_unbroken_word_after_short_label_keeps_word_boundary() {
        let long_name = "W".repeat(CARD_WRAP_COLUMNS);
        let label = format!("入口 {long_name}");
        let wrapped = wrap_card_text(&label);
        assert_eq!(wrapped.first().map(String::as_str), Some("入口"));
        assert_eq!(wrapped[1..].concat(), long_name);
        assert!(wrapped
            .iter()
            .all(|line| card_columns(line) <= CARD_WRAP_COLUMNS));
    }
    #[test]
    fn nearest_two_other_tasks_only_use_actual_visible_targets() {
        let mut tracker = QuestTracker {
            active_quests: (1..=5).map(quest).collect(),
        };
        for q in &mut tracker.active_quests {
            q.objectives.push(QuestObjective {
                objective_id: "kill".into(),
                text: format!("Kill Monster{}", q.quest_index),
                current: 0,
                target: 2,
            });
        }
        let mut entities = EntityModelSet::default();
        for id in 1..=4 {
            entities.entities.push(crate::entities::EntityModel {
                object_id: id.to_string(),
                kind: crate::entities::EntityKind::Monster,
                name: format!("Monster{id}"),
                x: id,
                y: 0,
                level: None,
                direction: None,
            });
        }
        assert_eq!(
            nearby_indices(1, &tracker, &entities, &MapModel::default(), None),
            vec![2, 3]
        );
        assert_eq!(
            place(
                &tracker.active_quests[4],
                &tracker,
                &entities,
                &MapModel::default(),
                None
            ),
            Place::Unknown
        );
    }
    #[test]
    fn remote_map_labels_require_known_map_and_authored_target() {
        let maps = &mir2_game_data::crystal_respawn_manifest_ref().maps;
        let bichon = maps.iter().find(|m| m.map_file_name == "0").unwrap();
        let wooma = maps.iter().find(|m| m.map_file_name == "D022").unwrap();
        assert_eq!(
            authored_other_map(&quest(2_110_019), Some(wooma.map_index)),
            None
        );
        assert!(
            authored_other_map(&quest(2_110_019), Some(bichon.map_index))
                .unwrap()
                .contains(&crate::player_text::name(&wooma.map_title))
        );
        assert_eq!(authored_other_map(&quest(2_110_019), None), None);
        assert_eq!(
            authored_other_map(&quest(12345), Some(bichon.map_index)),
            None
        );
    }

    #[test]
    fn oma_route_card_uses_the_imported_bichon_entrance_and_navigation_intent() {
        let bichon = mir2_game_data::crystal_respawn_manifest_ref()
            .maps
            .iter()
            .find(|map| map.map_file_name == "0")
            .unwrap();
        let mut world = World::new();
        let tracker = QuestTracker {
            active_quests: vec![quest(2_110_010)],
        };
        let state = QuestUiState::default();
        let map = MapModel {
            center_x: 288,
            center_y: 616,
            ..default()
        };
        let big_map = BigMapModel {
            current_map_index: Some(bichon.map_index),
            reset_epoch: 41,
            ..default()
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.spawn_empty().with_children(|parent| {
            assert!(render(
                parent,
                &tracker,
                &state,
                None,
                &EntityModelSet::default(),
                &map,
                Some(&big_map),
                "Warrior",
                &test_supplies()
            ));
        });
        queue.apply(&mut world);
        let text = world
            .query::<&Text>()
            .iter(&world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains("当前位置 (288,616)"));
        assert!(text.contains("入口 (147,33)"));
        assert!(text.contains("半兽人洞穴一层"));
        assert!(world
            .query::<&QuestUiButton>()
            .iter(&world)
            .any(|button| matches!(button,
                QuestUiButton::NavigateQuestRoute(crate::quest_ui::QuestRouteNavigationIntent {
                    target: QuestRouteTarget::Entrance,
                    quest_index: 2_110_010, reset_epoch: 41, map_index, x: 147, y: 33,
                }) if *map_index == bichon.map_index
            )));
    }

    #[test]
    fn d401_route_card_displays_navigation_feedback_with_its_outcome_color() {
        let d401 = mir2_game_data::crystal_respawn_manifest_ref()
            .maps
            .iter()
            .find(|map| map.map_file_name == "D401")
            .expect("imported D401 map");
        assert_eq!(d401.map_index, 47);
        let tracker = QuestTracker {
            active_quests: vec![quest(2_110_010)],
        };
        let map = MapModel {
            center_x: 23,
            center_y: 176,
            ..default()
        };
        let big_map = BigMapModel {
            current_map_index: Some(d401.map_index),
            reset_epoch: 41,
            ..default()
        };

        for (message, is_error, expected_color) in [
            ("前往入口 (24,182)", false, FEEDBACK_OK),
            ("入口引导已更新，请使用当前任务路线", true, FEEDBACK_ERR),
        ] {
            let state = QuestUiState {
                feedback: Some(QuestFeedback {
                    message: message.into(),
                    is_error,
                }),
                ..default()
            };
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, &world);
            commands.spawn_empty().with_children(|parent| {
                assert!(render(
                    parent,
                    &tracker,
                    &state,
                    None,
                    &EntityModelSet::default(),
                    &map,
                    Some(&big_map),
                    "Warrior",
                    &test_supplies()
                ));
            });
            queue.apply(&mut world);
            assert!(world
                .query::<(&Text, &TextColor)>()
                .iter(&world)
                .any(|(text, color)| text.0 == message && color.0 == expected_color));
        }
    }

    #[test]
    fn authored_cat_region_is_nearby_without_live_entities_and_completed_or_absent_progress_is_not_inferred(
    ) {
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
                label: "狩猎区域 · 钉耙猫 (340,550) · 64 格".into(),
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

    #[test]
    fn authored_target_oma_arrival_card_keeps_navigation_when_oma_appears() {
        let mut arrival = quest(2_110_009);
        arrival.title = "Enter Oma Cave".into();
        arrival.objectives.push(QuestObjective {
            objective_id: "2110009:0".into(),
            text: "Reach the Oma Cave entrance".into(),
            current: 0,
            target: 1,
        });
        let tracker = QuestTracker {
            active_quests: vec![arrival],
        };
        let map = MapModel {
            center_x: 429,
            center_y: 82,
            ..default()
        };
        let big_map = BigMapModel {
            current_map_index: Some(1),
            reset_epoch: 42,
            ..default()
        };
        for visible in [false, true, false] {
            let mut entities = EntityModelSet::default();
            if visible {
                entities.entities.push(crate::entities::EntityModel {
                    object_id: "20".into(),
                    kind: crate::entities::EntityKind::Monster,
                    name: "Oma".into(),
                    x: 420,
                    y: 91,
                    level: None,
                    direction: None,
                });
            }
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, &world);
            commands.spawn_empty().with_children(|parent| {
                assert!(render(
                    parent,
                    &tracker,
                    &QuestUiState::default(),
                    None,
                    &entities,
                    &map,
                    Some(&big_map),
                    "Warrior",
                    &test_supplies()
                ));
            });
            queue.apply(&mut world);
            assert!(
                world
                    .query::<&QuestUiButton>()
                    .iter(&world)
                    .any(|button| matches!(
                        button,
                        QuestUiButton::NavigateQuestRoute(
                            crate::quest_ui::QuestRouteNavigationIntent {
                                target: QuestRouteTarget::Entrance,
                                quest_index: 2_110_009,
                                reset_epoch: 42,
                                map_index: 1,
                                x: 147,
                                y: 33,
                            }
                        )
                    )),
                "visible Oma={visible} must not remove the arrival route button"
            );
            let text = world
                .query::<&Text>()
                .iter(&world)
                .map(|text| text.0.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("入口 (147,33)"));
            assert!(!text.contains("Oma (420,91)"));
            assert!(text.contains("抵达半兽人洞穴入口  0/1"));
        }
    }

    #[test]
    fn authored_target_map_takes_priority_over_a_monster_on_the_wrong_map() {
        let mut hunt = quest(2_110_010);
        hunt.objectives.push(QuestObjective {
            objective_id: "2110010:0".into(),
            text: "Skeleton".into(),
            current: 0,
            target: 4,
        });
        let tracker = QuestTracker {
            active_quests: vec![hunt.clone()],
        };
        let entities = EntityModelSet {
            entities: vec![crate::entities::EntityModel {
                object_id: "21".into(),
                kind: crate::entities::EntityKind::Monster,
                name: "Skeleton".into(),
                x: 420,
                y: 91,
                level: None,
                direction: None,
            }],
        };
        let map = MapModel {
            center_x: 429,
            center_y: 82,
            ..default()
        };
        let mut big_map = BigMapModel {
            current_map_index: Some(1),
            ..default()
        };
        assert!(matches!(
            place(&hunt, &tracker, &entities, &map, Some(&big_map)),
            Place::Route(_)
        ));
        big_map.set_current_map(39);
        assert!(matches!(
            place(&hunt, &tracker, &entities, &map, Some(&big_map)),
            Place::Current { .. }
        ));
        assert_eq!(
            place(&hunt, &tracker, &entities, &map, None),
            Place::Unknown,
            "unknown map identity must not claim a map-specific monster is nearby"
        );
    }

    #[test]
    fn hunt_navigation_card_keeps_region_action_with_or_without_visible_skeletons() {
        for visible in [false, true, false] {
            let mut hunt = quest(2_110_010);
            hunt.title = "Push back the skeletons".into();
            hunt.objectives.push(QuestObjective {
                objective_id: "2110010:0".into(),
                text: "Defeat 4 Skeleton.".into(),
                current: 0,
                target: 4,
            });
            let tracker = QuestTracker {
                active_quests: vec![hunt],
            };
            let map = MapModel {
                center_x: 211,
                center_y: 320,
                ..default()
            };
            let big_map = BigMapModel {
                current_map_index: Some(39),
                reset_epoch: 12,
                ..default()
            };
            let mut entities = EntityModelSet::default();
            if visible {
                entities.entities.push(crate::entities::EntityModel {
                    object_id: "22".into(),
                    kind: crate::entities::EntityKind::Monster,
                    name: "Skeleton".into(),
                    x: 221,
                    y: 320,
                    level: None,
                    direction: None,
                });
            }
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, &world);
            commands.spawn_empty().with_children(|parent| {
                assert!(render(
                    parent,
                    &tracker,
                    &QuestUiState::default(),
                    None,
                    &entities,
                    &map,
                    Some(&big_map),
                    "Warrior",
                    &test_supplies()
                ));
            });
            queue.apply(&mut world);
            assert!(
                world
                    .query::<&QuestUiButton>()
                    .iter(&world)
                    .any(|button| matches!(
                        button,
                        QuestUiButton::NavigateQuestRoute(
                            crate::quest_ui::QuestRouteNavigationIntent {
                                target: QuestRouteTarget::HuntRegion { radius: 30, .. },
                                quest_index: 2_110_010,
                                reset_epoch: 12,
                                map_index: 39,
                                x: 250,
                                y: 260,
                                ..
                            }
                        )
                    )),
                "visible Skeleton={visible} must retain the hunting-area action"
            );
            let text = world
                .query::<&Text>()
                .iter(&world)
                .map(|text| text.0.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("前往狩猎区域 · 自动寻路"));
            assert!(text.contains("消灭 4 只骷髅  0/4"));
        }
    }
}
