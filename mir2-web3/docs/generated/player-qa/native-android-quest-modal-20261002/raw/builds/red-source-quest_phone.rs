//! Optional phone presentation of the shared diary/detail, never quest rules.
use super::*;

/// Android supplies logical workspace dimensions and the inverse stage scale.
/// Absence preserves the source desktop renderer and its default geometry.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct PhoneQuestPresentation {
    pub workspace: Vec2,
    pub authored_unit: f32,
}

/// No-op ResMut access in the shared controller marks resources changed every
/// frame. Retain phone entities across those frames so a press can reach its
/// release. This compares view inputs only; default desktop churn is unchanged.
#[derive(PartialEq)]
pub(super) struct RenderSnapshot {
    presentation: PhoneQuestPresentation,
    locale: u64,
    tracker: QuestTracker,
    journey: Option<JourneyView>,
    player: crate::read_model::PlayerStats,
    diary_open: bool,
    detail: Option<i32>,
    selected: Option<i32>,
    reward: Option<i32>,
    tracked: Vec<i32>,
    primary: Option<i32>,
    tab: GuidedDiaryTab,
    page: usize,
    collapsed: Vec<String>,
    graduation: Option<GraduationDirection>,
    action: Option<(QuestUiButton, bool)>,
    abandon_pending: bool,
}

impl RenderSnapshot {
    pub(super) fn capture(
        presentation: &PhoneQuestPresentation,
        tracker: &QuestTracker,
        journey: Option<&JourneyView>,
        player: &crate::read_model::PlayerStats,
        diary_open: bool,
        state: &QuestUiState,
        guidance: &QuestGuidance,
        pending: &PendingOperations,
    ) -> Self {
        let detail = state.detail_quest(tracker);
        Self {
            presentation: *presentation,
            locale: crate::native_i18n::revision(),
            tracker: tracker.clone(),
            journey: journey.cloned(),
            player: player.clone(),
            diary_open,
            detail: detail.map(|quest| quest.quest_index),
            selected: state.selected_quest_index,
            reward: state.selected_reward_index,
            tracked: state.tracked_quest_indices.clone(),
            primary: state.pinned_primary_quest_index,
            tab: state.diary_tab,
            page: state.diary_page,
            collapsed: state.collapsed_groups.clone(),
            graduation: state.selected_graduation_direction,
            action: detail.map(|quest| {
                let spec = quest_detail_primary_spec(quest, guidance, state, pending);
                (spec.action, spec.enabled)
            }),
            abandon_pending: detail.is_some_and(|quest| {
                pending.contains(&PendingOperationKey::QuestAbandon {
                    quest_index: quest.quest_index,
                })
            }),
        }
    }
}

impl PhoneQuestPresentation {
    pub(super) fn is_valid(&self) -> bool {
        self.workspace.is_finite()
            && self.workspace.min_element() >= 160.0
            && self.authored_unit.is_finite()
            && self.authored_unit > 0.0
    }

    pub(super) fn apply_panel(&self, node: &mut Node, detail: bool, pair: bool) {
        let width = if pair {
            (self.workspace.x - 12.0) / 2.0
        } else {
            self.workspace.x
        };
        let unit = self.authored_unit;
        node.left = Val::Px(if detail && pair {
            (width + 12.0) * unit
        } else {
            0.0
        });
        node.top = Val::Px(0.0);
        node.width = Val::Px(width * unit);
        node.height = Val::Px(self.workspace.y * unit);
        node.min_width = node.width;
        node.max_width = node.width;
        node.min_height = node.height;
        node.max_height = node.height;
        node.flex_direction = FlexDirection::Column;
        node.padding = UiRect::all(Val::Px(8.0 * unit));
        node.row_gap = Val::Px(6.0 * unit);
    }
}

/// Markers let the Android host report actual computed geometry, not estimates.
#[derive(Component)]
pub struct PhoneQuestText;
#[derive(Component)]
pub struct PhoneQuestControl;
#[derive(Component)]
pub struct PhoneQuestScrollArea(ScrollKey);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScrollKey {
    Diary(GuidedDiaryTab, usize),
    Detail(i32),
}

#[derive(Component)]
pub(super) struct PhoneQuestButton {
    scroll: Option<ScrollKey>,
}

#[derive(Component)]
pub(super) struct ScrollStep {
    key: ScrollKey,
    direction: f32,
}

#[derive(Clone, Copy)]
struct Gesture {
    touch: Option<u64>,
    button: Option<Entity>,
    scroll: Option<Entity>,
    start: Vec2,
    last: Vec2,
    dragged: bool,
    presentation: PhoneQuestPresentation,
}

#[derive(Resource, Default)]
pub(super) struct PhoneInputState {
    pub(super) clicks: Vec<QuestUiButton>,
    gesture: Option<Gesture>,
    diary: Option<(ScrollKey, Vec2)>,
    detail: Option<(ScrollKey, Vec2)>,
    reset: Option<u64>,
}

impl PhoneInputState {
    fn offset(&self, key: ScrollKey) -> Vec2 {
        match key {
            ScrollKey::Diary(..) => self.diary,
            ScrollKey::Detail(..) => self.detail,
        }
        .filter(|(saved, _)| *saved == key)
        .map(|(_, offset)| offset)
        .unwrap_or_default()
    }
    fn remember(&mut self, key: ScrollKey, offset: Vec2) {
        let slot = match key {
            ScrollKey::Diary(..) => &mut self.diary,
            ScrollKey::Detail(..) => &mut self.detail,
        };
        *slot = Some((key, offset));
    }
}

/// Native touch/mouse release is converted to the existing shared action enum.
/// Scrolling never edits a tracker, pending operation, reward or quest status.
pub(super) fn pointer_input(
    presentation: Option<Res<PhoneQuestPresentation>>,
    mut state: ResMut<PhoneInputState>,
    shell: Res<NativeShellModel>,
    player: Res<NativePlayerUiState>,
    quest_state: Res<QuestUiState>,
    dialog: Res<NpcDialogModel>,
    reset: Res<SessionResetRevision>,
    touches: Option<Res<Touches>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window>,
    controls: Query<(
        Entity,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
        &PhoneQuestButton,
        Option<&QuestUiButton>,
        Option<&ScrollStep>,
    )>,
    mut areas: Query<(
        Entity,
        &PhoneQuestScrollArea,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
        &mut ScrollPosition,
    )>,
) {
    state.clicks.clear();
    if state.reset != Some(reset.0) {
        *state = PhoneInputState {
            reset: Some(reset.0),
            ..default()
        };
    }
    let Some(presentation) = presentation.as_deref().filter(|phone| phone.is_valid()) else {
        return;
    };
    let Ok(window) = windows.single() else {
        state.gesture = None;
        return;
    };
    if shell.screen != NativeShellScreen::InGame {
        state.gesture = None;
        state.diary = None;
        state.detail = None;
        return;
    }
    if !window.focused
        || quest_state.blocks_world_input()
        || player.blocks_world_action(dialog.is_open, false)
    {
        state.gesture = None;
        return;
    }
    // Actual layout clamps offsets after content/viewport changes. Remember the
    // clamped value so subsequent model renders do not jump back to the top.
    for (_, area, computed, _, _) in &areas {
        state.remember(
            area.0,
            computed.scroll_position * computed.inverse_scale_factor,
        );
    }
    let density = window.scale_factor();
    let hit_button = |position: Vec2| {
        controls
            .iter()
            .find_map(|(entity, node, transform, button, _, _)| {
                let in_clip = button.scroll.is_none_or(|key| {
                    areas.iter().any(|(_, area, clip, transform, _)| {
                        area.0 == key && clip.contains_point(*transform, position)
                    })
                });
                (in_clip && node.contains_point(*transform, position)).then_some(entity)
            })
    };
    if state.gesture.is_none() {
        let start = touches
            .as_deref()
            .and_then(|touches| {
                touches.iter_just_pressed().find_map(|touch| {
                    let position = touch.start_position() * density;
                    let scroll = areas.iter().find_map(|(entity, _, node, transform, _)| {
                        node.contains_point(*transform, position).then_some(entity)
                    });
                    let button = hit_button(position);
                    (scroll.is_some() || button.is_some()).then_some((
                        Some(touch.id()),
                        position,
                        button,
                        scroll,
                    ))
                })
            })
            .or_else(|| {
                if touches
                    .as_deref()
                    .is_some_and(|touches| touches.iter().next().is_some())
                    || !mouse
                        .as_deref()
                        .is_some_and(|buttons| buttons.just_pressed(MouseButton::Left))
                {
                    return None;
                }
                let position = window.cursor_position()? * density;
                let button = hit_button(position);
                let scroll = areas.iter().find_map(|(entity, _, node, transform, _)| {
                    node.contains_point(*transform, position).then_some(entity)
                });
                (scroll.is_some() || button.is_some()).then_some((None, position, button, scroll))
            });
        if let Some((touch, position, button, scroll)) = start {
            state.gesture = Some(Gesture {
                touch,
                button,
                scroll,
                start: position,
                last: position,
                dragged: false,
                presentation: *presentation,
            });
        }
    }
    let Some(mut gesture) = state.gesture else {
        return;
    };
    if gesture.presentation != *presentation {
        state.gesture = None;
        return;
    }
    let (position, ended, canceled) = if let Some(id) = gesture.touch {
        let Some(touches) = touches.as_deref() else {
            state.gesture = None;
            return;
        };
        if let Some(touch) = touches.get_pressed(id) {
            (touch.position() * density, false, false)
        } else if let Some(touch) = touches.iter_just_released().find(|touch| touch.id() == id) {
            (touch.position() * density, true, false)
        } else {
            (gesture.last, true, true)
        }
    } else {
        let pressed = mouse
            .as_deref()
            .is_some_and(|buttons| buttons.pressed(MouseButton::Left));
        let released = mouse
            .as_deref()
            .is_some_and(|buttons| buttons.just_released(MouseButton::Left));
        (
            window
                .cursor_position()
                .map(|point| point * density)
                .unwrap_or(gesture.last),
            !pressed || released,
            !pressed && !released,
        )
    };
    let released_hit = hit_button(position);
    gesture.dragged |= position.distance(gesture.start) > 8.0 * density;
    if gesture.dragged && !canceled {
        if let Some((_, area, computed, _, mut offset)) =
            gesture.scroll.and_then(|entity| areas.get_mut(entity).ok())
        {
            let max = (computed.content_size - computed.size).max(Vec2::ZERO)
                * computed.inverse_scale_factor;
            offset.0.y = (offset.0.y
                - (position.y - gesture.last.y) * computed.inverse_scale_factor)
                .clamp(0.0, max.y);
            state.remember(area.0, offset.0);
        }
    }
    gesture.last = position;
    if ended {
        if !gesture.dragged && !canceled {
            if let Some(entity) = gesture
                .button
                .filter(|entity| released_hit == Some(*entity))
            {
                if let Ok((_, _, _, _, action, step)) = controls.get(entity) {
                    if let Some(action) = action {
                        state.clicks.push(action.clone());
                    }
                    if let Some(step) = step {
                        if let Some((_, area, computed, _, mut offset)) = areas
                            .iter_mut()
                            .find(|(_, area, _, _, _)| area.0 == step.key)
                        {
                            let max = (computed.content_size - computed.size).max(Vec2::ZERO)
                                * computed.inverse_scale_factor;
                            offset.0.y = (offset.0.y
                                + step.direction
                                    * computed.size.y
                                    * computed.inverse_scale_factor
                                    * 0.75)
                                .clamp(0.0, max.y);
                            state.remember(area.0, offset.0);
                        }
                    }
                }
            }
        }
        state.gesture = None;
    } else {
        state.gesture = Some(gesture);
    }
}

fn text(parent: &mut ChildSpawnerCommands, value: &str, unit: f32, heading: bool) {
    parent.spawn((
        PhoneQuestText,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            flex_shrink: 0.0,
            ..default()
        },
        Text::new(value),
        TextFont {
            font: FontSource::SystemUi,
            font_size: FontSize::Px(if heading { 16.0 } else { 14.0 } * unit),
            ..default()
        },
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        TextColor(if heading { PANEL_HIGHLIGHT } else { PANEL_TEXT }),
        FocusPolicy::Pass,
    ));
}

fn control_node(unit: f32) -> Node {
    Node {
        width: Val::Percent(100.0),
        min_width: Val::Px(48.0 * unit),
        min_height: Val::Px(48.0 * unit),
        flex_shrink: 0.0,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::all(Val::Px(8.0 * unit)),
        border: UiRect::all(Val::Px(unit)),
        ..default()
    }
}

fn button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: QuestUiButton,
    enabled: bool,
    key: Option<ScrollKey>,
    unit: f32,
) -> Entity {
    let mut node = parent.spawn((
        PhoneQuestControl,
        control_node(unit),
        QuestUiButtonVisual { enabled },
        BackgroundColor(if enabled { BUTTON_BG } else { BUTTON_DISABLED }),
        BorderColor::all(PANEL_HIGHLIGHT),
        FocusPolicy::Block,
    ));
    if enabled {
        node.insert((
            Button,
            Interaction::None,
            PhoneQuestButton { scroll: key },
            action,
        ));
    }
    let entity = node.id();
    node.with_children(|parent| text(parent, &crate::player_text::text(label), unit, false));
    entity
}

fn header(parent: &mut ChildSpawnerCommands, label: &str, close: QuestUiButton, unit: f32) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(48.0 * unit),
            flex_shrink: 0.0,
            column_gap: Val::Px(8.0 * unit),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_width: Val::Px(0.0),
                    ..default()
                })
                .with_children(|parent| text(parent, &crate::player_text::text(label), unit, true));
            parent
                .spawn(Node {
                    width: Val::Px(48.0 * unit),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|parent| {
                    button(parent, "×", close, true, None, unit);
                });
        });
}

fn scroll_content(
    parent: &mut ChildSpawnerCommands,
    key: ScrollKey,
    unit: f32,
    memory: Option<&PhoneInputState>,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            PhoneQuestScrollArea(key),
            ScrollPosition(memory.map(|state| state.offset(key)).unwrap_or_default()),
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(QUEST_LOG_BG),
            FocusPolicy::Block,
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    padding: UiRect::all(Val::Px(8.0 * unit)),
                    row_gap: Val::Px(8.0 * unit),
                    ..default()
                })
                .with_children(content);
        });
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            column_gap: Val::Px(8.0 * unit),
            ..default()
        })
        .with_children(|parent| {
            for (label, direction) in [("▲", -1.0), ("▼", 1.0)] {
                parent
                    .spawn((
                        PhoneQuestControl,
                        PhoneQuestButton { scroll: None },
                        ScrollStep { key, direction },
                        Node {
                            width: Val::Percent(50.0),
                            flex_grow: 1.0,
                            flex_basis: Val::Px(0.0),
                            ..control_node(unit)
                        },
                        Button,
                        Interaction::None,
                        QuestUiButtonVisual { enabled: true },
                        BackgroundColor(BUTTON_BG),
                        BorderColor::all(PANEL_HIGHLIGHT),
                        FocusPolicy::Block,
                    ))
                    .with_children(|parent| text(parent, label, unit, false));
            }
        });
}

fn quest_row(
    parent: &mut ChildSpawnerCommands,
    quest: &Quest,
    state: &QuestUiState,
    key: ScrollKey,
    unit: f32,
    current: bool,
) {
    let label = format!(
        "{}{} · {}\n{}级 · {}",
        if current { "● " } else { "" },
        crate::player_text::quest_title(quest.quest_index, &quest.title),
        crate::player_text::text(quest_diary_status_label(quest)),
        quest.min_level_needed.max(0),
        crate::player_text::text(quest.group.as_deref().unwrap_or("General"))
    );
    let entity = button(
        parent,
        &label,
        QuestUiButton::SelectQuest {
            quest_index: quest.quest_index,
        },
        true,
        Some(key),
        unit,
    );
    parent.commands().entity(entity).insert((
        QuestDiaryRow {
            quest_index: quest.quest_index,
        },
        RelativeCursorPosition::default(),
    ));
    button(
        parent,
        if state.is_tracked(quest.quest_index) {
            "取消跟踪"
        } else {
            "跟踪"
        },
        QuestUiButton::TrackQuest {
            quest_index: quest.quest_index,
        },
        quest.status.is_active(),
        Some(key),
        unit,
    );
}

fn graduation(
    parent: &mut ChildSpawnerCommands,
    journey: Option<&JourneyView>,
    state: &QuestUiState,
    key: ScrollKey,
    unit: f32,
) {
    if let Some(graduation) = journey.and_then(|view| view.graduation.as_ref()) {
        text(
            parent,
            &crate::player_text::text(&graduation.title),
            unit,
            true,
        );
        for option in &graduation.options {
            button(
                parent,
                &format!(
                    "{} {}",
                    if state.selected_graduation_direction == Some(option.direction) {
                        "●"
                    } else {
                        "+"
                    },
                    crate::player_text::text(&option.title)
                ),
                QuestUiButton::SelectGraduationDirection {
                    direction: option.direction,
                },
                true,
                Some(key),
                unit,
            );
            text(
                parent,
                &crate::player_text::text(&option.summary),
                unit,
                false,
            );
            text(
                parent,
                &crate::player_text::text(&option.instruction),
                unit,
                false,
            );
        }
    }
}

pub(super) fn render_diary(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    guidance: &QuestGuidance,
    journey: Option<&JourneyView>,
    state: &QuestUiState,
    presentation: &PhoneQuestPresentation,
    memory: Option<&PhoneInputState>,
) {
    let unit = presentation.authored_unit;
    let key = ScrollKey::Diary(state.diary_tab, state.diary_page);
    header(parent, "任务日志", QuestUiButton::CloseQuestLog, unit);
    scroll_content(parent, key, unit, memory, |parent| {
        if guidance.profile_name() == Some("newcomer-v2") {
            for tab in GuidedDiaryTab::ALL {
                let count = guided_diary_quests(tracker, Some(guidance), journey, tab).len();
                button(
                    parent,
                    &format!(
                        "{} {} {count}",
                        if state.diary_tab == tab { "●" } else { "" },
                        crate::player_text::text(tab.label())
                    ),
                    QuestUiButton::SelectGuidedDiaryTab(tab),
                    true,
                    Some(key),
                    unit,
                );
            }
            if let Some(view) = journey {
                text(
                    parent,
                    &crate::player_text::text(&view.chapter_title),
                    unit,
                    true,
                );
                if let (Some(done), Some(total)) = (view.completed_count, view.quest_count) {
                    text(
                        parent,
                        &format!("{}: {done}/{total}", crate::player_text::text("主线进度")),
                        unit,
                        false,
                    );
                }
            }
            let quests = guided_diary_quests(tracker, Some(guidance), journey, state.diary_tab);
            let page_count = quests.len().div_ceil(GUIDED_DIARY_PAGE_SIZE).max(1);
            let page = state.diary_page.min(page_count - 1);
            for quest in quests
                .iter()
                .skip(page * GUIDED_DIARY_PAGE_SIZE)
                .take(GUIDED_DIARY_PAGE_SIZE)
            {
                let current = journey
                    .and_then(|view| view.next.as_ref())
                    .is_some_and(|next| next.quest_id == quest.quest_index);
                quest_row(parent, quest, state, key, unit, current);
            }
            if state.diary_tab == GuidedDiaryTab::Main {
                graduation(parent, journey, state, key, unit);
            }
            if quests.is_empty() {
                text(
                    parent,
                    &crate::player_text::text("暂无可显示的主线，完成前置任务后刷新。"),
                    unit,
                    false,
                );
            }
            text(parent, &format!("{}/{page_count}", page + 1), unit, false);
            button(
                parent,
                "上一页",
                QuestUiButton::GuidedDiaryPrevious,
                page > 0,
                Some(key),
                unit,
            );
            button(
                parent,
                "下一页",
                QuestUiButton::GuidedDiaryNext,
                page + 1 < page_count,
                Some(key),
                unit,
            );
        } else {
            let groups = quest_diary_groups(tracker, Some(guidance));
            let count = groups.iter().map(|group| group.quests.len()).sum::<usize>();
            text(
                parent,
                &format!(
                    "{}: {count}/{QUEST_DIARY_MAX_CURRENT}",
                    crate::player_text::text("任务列表")
                ),
                unit,
                false,
            );
            for group in groups {
                let collapsed = state.is_group_collapsed(&group.name);
                button(
                    parent,
                    &format!(
                        "{} {}",
                        if collapsed { "+" } else { "−" },
                        crate::player_text::text(&group.name)
                    ),
                    QuestUiButton::ToggleQuestGroup {
                        group: group.name.clone(),
                    },
                    true,
                    Some(key),
                    unit,
                );
                if !collapsed {
                    for quest in group.quests {
                        quest_row(parent, quest, state, key, unit, false);
                    }
                }
            }
            graduation(parent, journey, state, key, unit);
        }
    });
}

pub(super) fn render_detail(
    parent: &mut ChildSpawnerCommands,
    quest: &Quest,
    guidance: &QuestGuidance,
    state: &QuestUiState,
    pending: &PendingOperations,
    presentation: &PhoneQuestPresentation,
    memory: Option<&PhoneInputState>,
    player: &crate::read_model::PlayerStats,
    assets: Option<&AssetServer>,
) {
    let unit = presentation.authored_unit;
    let key = ScrollKey::Detail(quest.quest_index);
    header(parent, "任务详情", QuestUiButton::CloseQuestDetail, unit);
    scroll_content(parent, key, unit, memory, |parent| {
        for line in quest_detail_lines(
            quest,
            Some(guidance),
            player.class_name.as_deref().unwrap_or(""),
        ) {
            if line.kind != QuestDetailLineKind::Blank {
                text(
                    parent,
                    &line.text,
                    unit,
                    matches!(
                        line.kind,
                        QuestDetailLineKind::Title | QuestDetailLineKind::Heading
                    ),
                );
            }
        }
        text(parent, &crate::player_text::text("奖励"), unit, true);
        for reward in &quest.rewards {
            match reward {
                crate::quest_model::QuestReward::Experience { amount } => text(
                    parent,
                    &format!("{}: {amount}", crate::player_text::text("Experience")),
                    unit,
                    false,
                ),
                crate::quest_model::QuestReward::Gold { amount } => text(
                    parent,
                    &format!("{}: {amount}", crate::player_text::text("Gold")),
                    unit,
                    false,
                ),
                _ => {}
            }
        }
        for selectable in [false, true] {
            for reward in quest.rewards.iter().filter(|reward| matches!(reward,
                crate::quest_model::QuestReward::Item { selection_index, .. } if selection_index.is_some() == selectable))
                .filter(|reward| selectable || fixed_reward_is_visible(reward, guidance)).take(5) {
                let crate::quest_model::QuestReward::Item { name, quantity, selection_index, icon, tooltip_source, .. } = reward else { continue };
                let label = format!("{} {name} × {quantity}", if selection_index.is_some() && *selection_index == state.selected_reward_index { "●" } else { "" });
                if let Some(reward_index) = selection_index {
                    let entity = button(parent, &label, QuestUiButton::SelectReward { quest_index: quest.quest_index, reward_index: *reward_index }, true, Some(key), unit);
                    if let Some(document) = crystal_item_tooltip_document_from_source(name,
                        icon.and_then(|icon| u16::try_from(icon).ok()).unwrap_or_default(), *quantity, tooltip_source.as_ref(), player) {
                        parent.commands().entity(entity).insert(CrystalItemHint(document));
                    }
                } else { text(parent, &label, unit, false); }
                if let (Some(assets), Some(icon)) = (assets, icon) {
                    parent.spawn((Node { width: Val::Px(32.0 * unit), height: Val::Px(32.0 * unit), flex_shrink: 0.0, ..default() },
                        ImageNode { image: assets.load(format!("original-ui/Items/{icon}.png")), ..default() }));
                }
            }
        }
        if guidance.is_enabled() && quest.status.is_active() {
            button(
                parent,
                if state.pinned_primary_quest_index == Some(quest.quest_index) {
                    "当前引导"
                } else {
                    "设为当前"
                },
                QuestUiButton::MakePrimary {
                    quest_index: quest.quest_index,
                },
                true,
                Some(key),
                unit,
            );
        }
        button(
            parent,
            if state.is_tracked(quest.quest_index) {
                "取消跟踪"
            } else {
                "跟踪"
            },
            QuestUiButton::TrackQuest {
                quest_index: quest.quest_index,
            },
            quest.status.is_active(),
            Some(key),
            unit,
        );
        let primary = quest_detail_primary_spec(quest, guidance, state, pending);
        let label = match primary.action {
            QuestUiButton::AcceptQuest { .. } => "Accept",
            QuestUiButton::FinishQuest { .. } | QuestUiButton::PrepareQuestFinish { .. } => "交付",
            _ => "Share",
        };
        button(
            parent,
            label,
            primary.action,
            primary.enabled,
            Some(key),
            unit,
        );
        button(
            parent,
            "Abandon",
            QuestUiButton::AbandonQuest {
                quest_index: quest.quest_index,
            },
            can_abandon_quest(quest)
                && !pending.contains(&PendingOperationKey::QuestAbandon {
                    quest_index: quest.quest_index,
                }),
            Some(key),
            unit,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_ui_core::state::{UiPanel, UiScreen};

    fn fixture() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .insert_resource(PhoneQuestPresentation {
                workspace: Vec2::new(457.0, 361.0),
                authored_unit: 1.0 / 0.51171875,
            })
            .add_plugins(Mir2QuestUiPlugin);
        app.update();
        let world = app.world_mut();
        let mut player = world.resource_mut::<NativePlayerUiState>();
        player.core.screen = UiScreen::InGame;
        player.core.panel = UiPanel::QuestLog;
        world
            .resource_mut::<QuestTracker>()
            .active_quests
            .push(Quest {
                quest_index: 7,
                accept_npc_index: Some(10),
                finish_npc_index: Some(11),
                title: "Patrol fixture, keep the full server title".into(),
                npc_name: Some("Guard".into()),
                group: Some("BichonProvince".into()),
                min_level_needed: 1,
                detail: default(),
                status: crate::quest_model::QuestStatus::InProgress,
                objectives: Vec::new(),
                rewards: Vec::new(),
                unknown_text: None,
            });
        app.update();
        app
    }

    #[test]
    fn phone_quest_renderer_reflows_actual_rows_instead_of_scaling_desktop_rows() {
        let mut app = fixture();
        let world = app.world_mut();
        let unit = world.resource::<PhoneQuestPresentation>().authored_unit;
        let node = world
            .query::<(&Node, &QuestDiaryRow)>()
            .iter(world)
            .find(|(_, row)| row.quest_index == 7)
            .unwrap()
            .0;
        assert!(
            matches!(node.min_height, Val::Px(value) if value / unit >= 48.0),
            "The actual shared phone row needs a 48dp minimum, not the source 15px row"
        );
    }

    #[test]
    fn actual_phone_controls_and_text_keep_dp_metrics_in_both_independent_windows() {
        let mut app = fixture();
        app.world_mut()
            .resource_mut::<QuestUiState>()
            .select_quest(7);
        app.update();
        let world = app.world_mut();
        let unit = world.resource::<PhoneQuestPresentation>().authored_unit;
        let diary = world
            .query_filtered::<&Node, With<QuestLogPanel>>()
            .single(world)
            .unwrap();
        assert_eq!(diary.width, Val::Px((457.0 - 12.0) / 2.0 * unit));
        assert_eq!(diary.height, Val::Px(361.0 * unit));
        for node in world
            .query_filtered::<&Node, With<PhoneQuestControl>>()
            .iter(world)
        {
            assert!(matches!(node.min_height, Val::Px(value) if value / unit >= 48.0));
            assert!(matches!(node.min_width, Val::Px(value) if value / unit >= 48.0));
            assert_eq!(node.flex_shrink, 0.0);
        }
        for font in world
            .query_filtered::<&TextFont, With<PhoneQuestText>>()
            .iter(world)
        {
            assert!(matches!(font.font_size, FontSize::Px(value) if value / unit >= 14.0));
            assert!(matches!(font.font, FontSource::SystemUi));
        }
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0.contains("keep the full server title")));
        assert_eq!(
            world
                .query_filtered::<&Node, With<PhoneQuestScrollArea>>()
                .iter(world)
                .count(),
            2
        );
        assert!(world
            .query_filtered::<&Node, With<PhoneQuestScrollArea>>()
            .iter(world)
            .all(|node| node.overflow == Overflow::scroll_y()));
    }

    #[test]
    fn phone_child_entities_survive_no_op_shared_controller_frames() {
        let mut app = fixture();
        let world = app.world_mut();
        let before = world
            .query_filtered::<Entity, With<PhoneQuestControl>>()
            .iter(world)
            .collect::<Vec<_>>();
        app.update();
        app.update();
        let world = app.world_mut();
        let after = world
            .query_filtered::<Entity, With<PhoneQuestControl>>()
            .iter(world)
            .collect::<Vec<_>>();
        assert_eq!(
            before, after,
            "Press/release ownership must survive no-op resource change ticks"
        );
    }

    #[test]
    fn phone_quest_confirmation_uses_readable_text_and_48dp_controls() {
        let mut app = fixture();
        app.world_mut().resource_mut::<QuestUiState>().request_abandon_confirmation(7);
        app.update();
        let world = app.world_mut();
        let unit = world.resource::<PhoneQuestPresentation>().authored_unit;
        for action in [QuestUiButton::ConfirmAbandonQuest, QuestUiButton::CancelAbandonQuest] {
            let (entity, node) = world.query::<(Entity, &Node, &QuestUiButton)>()
                .iter(world).find_map(|(entity, node, found)| (*found == action).then_some((entity, node))).unwrap();
            assert!(world.get::<PhoneQuestControl>(entity).is_some(), "The real confirmation needs phone controls");
            assert!(matches!(node.min_height, Val::Px(value) if value / unit >= 48.0));
            assert!(matches!(node.min_width, Val::Px(value) if value / unit >= 48.0));
        }
        assert!(world.query::<(&PhoneQuestText, &TextFont)>().iter(world)
            .any(|(_, font)| matches!(font.font_size, FontSize::Px(value) if value / unit >= 14.0)));
    }

    #[test]
    fn phone_quest_confirmation_buttons_survive_unchanged_frames() {
        let mut app = fixture();
        app.world_mut().resource_mut::<QuestUiState>().request_abandon_confirmation(7);
        app.update();
        let action_entity = |app: &mut App| {
            let world = app.world_mut();
            world.query::<(Entity, &QuestUiButton)>().iter(world)
                .find_map(|(entity, action)| matches!(action, QuestUiButton::CancelAbandonQuest).then_some(entity)).unwrap()
        };
        let before = action_entity(&mut app);
        app.update(); app.update();
        assert_eq!(before, action_entity(&mut app), "A phone press must retain the same release target");
    }

    fn modal_gesture_fixture(alert: bool) -> (App, Entity) {
        let mut app = fixture();
        app.add_plugins(bevy::input::InputPlugin);
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            if alert { state.show_quest_alert("OFFLINE modal message: no server action"); }
            else { state.request_abandon_confirmation(7); }
        }
        app.update();
        let world = app.world_mut();
        let window = world.spawn(Window::default()).id();
        let button = world.query::<(Entity, &QuestUiButton)>().iter(world)
            .find_map(|(entity, action)| {
                (if alert { matches!(action, QuestUiButton::CloseQuestAlert) }
                 else { matches!(action, QuestUiButton::CancelAbandonQuest) }).then_some(entity)
            }).unwrap();
        // Bind native coordinates independently of a GPU layout. Tagging the
        // old control also isolates the previous modal input rejection.
        world.entity_mut(button).insert((
            PhoneQuestButton { scroll: None },
            ComputedNode { size: Vec2::new(180.0, 60.0), inverse_scale_factor: 1.0, ..default() },
            bevy::ui::UiGlobalTransform::from_xy(200.0, 210.0),
        ));
        (app, window)
    }

    #[test]
    fn phone_quest_confirmation_cancel_and_alert_close_use_shared_release_actions() {
        use bevy::input::touch::TouchPhase::*;
        for alert in [false, true] {
            let (mut app, window) = modal_gesture_fixture(alert);
            assert!(app.world().resource::<QuestUiState>().blocks_world_input());
            touch(&mut app, window, 1, Started, Vec2::new(200.0, 210.0)); app.update();
            assert!(app.world().resource::<QuestUiState>().blocks_world_input());
            touch(&mut app, window, 1, Ended, Vec2::new(200.0, 210.0)); app.update();
            assert!(!app.world().resource::<QuestUiState>().blocks_world_input(), "The modal must accept its own release without allowing the world");
            assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());
        }
    }

    fn gesture_fixture() -> (App, Entity, Entity) {
        let mut app = fixture();
        app.add_plugins(bevy::input::InputPlugin);
        let world = app.world_mut();
        let window = world.spawn(Window::default()).id();
        let button = world
            .query::<(Entity, &QuestUiButton)>()
            .iter(world)
            .find_map(|(entity, action)| {
                matches!(action, QuestUiButton::SelectQuest { quest_index: 7 }).then_some(entity)
            })
            .unwrap();
        world.entity_mut(button).insert((
            ComputedNode {
                size: Vec2::new(180.0, 60.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            bevy::ui::UiGlobalTransform::from_xy(200.0, 210.0),
        ));
        let area = world
            .query_filtered::<Entity, With<PhoneQuestScrollArea>>()
            .single(world)
            .unwrap();
        world.entity_mut(area).insert((
            ComputedNode {
                size: Vec2::new(400.0, 300.0),
                content_size: Vec2::new(400.0, 800.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            bevy::ui::UiGlobalTransform::from_xy(200.0, 240.0),
        ));
        (app, window, area)
    }

    fn touch(
        app: &mut App,
        window: Entity,
        id: u64,
        phase: bevy::input::touch::TouchPhase,
        point: Vec2,
    ) {
        app.world_mut()
            .write_message(bevy::input::touch::TouchInput {
                window,
                id,
                phase,
                position: point,
                force: None,
            });
    }

    #[test]
    fn phone_row_acts_on_release_through_the_existing_shared_controller() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, _) = gesture_fixture();
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 210.0));
        app.update();
        assert_eq!(
            app.world().resource::<QuestUiState>().detail_quest_index,
            None
        );
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 210.0));
        app.update();
        assert_eq!(
            app.world().resource::<QuestUiState>().detail_quest_index,
            Some(7)
        );
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn phone_swipe_scrolls_actual_scroll_position_without_selecting_a_quest() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, area) = gesture_fixture();
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 210.0));
        app.update();
        touch(&mut app, window, 1, Moved, Vec2::new(200.0, 120.0));
        app.update();
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 120.0));
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(area).unwrap().0.y, 90.0);
        assert_eq!(
            app.world().resource::<QuestUiState>().detail_quest_index,
            None
        );
    }

    #[test]
    fn canceled_owner_does_not_promote_a_second_finger_or_emit_a_click() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, _) = gesture_fixture();
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 210.0));
        app.update();
        touch(&mut app, window, 2, Started, Vec2::new(210.0, 210.0));
        app.update();
        touch(&mut app, window, 1, Canceled, Vec2::new(200.0, 210.0));
        app.update();
        touch(&mut app, window, 2, Ended, Vec2::new(210.0, 210.0));
        app.update();
        assert_eq!(
            app.world().resource::<QuestUiState>().detail_quest_index,
            None
        );
        assert!(app.world().resource::<PhoneInputState>().gesture.is_none());
    }

    #[test]
    fn reset_or_lost_focus_cancels_a_pending_phone_press() {
        use bevy::input::touch::TouchPhase::*;
        for lost_focus in [false, true] {
            let (mut app, window, _) = gesture_fixture();
            touch(&mut app, window, 1, Started, Vec2::new(200.0, 210.0));
            app.update();
            if lost_focus {
                app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
            } else {
                app.world_mut().resource_mut::<SessionResetRevision>().0 += 1;
            }
            touch(&mut app, window, 1, Ended, Vec2::new(200.0, 210.0));
            app.update();
            assert_eq!(
                app.world().resource::<QuestUiState>().detail_quest_index,
                None
            );
        }
    }

    #[test]
    fn phone_presentation_does_not_offer_an_unauthorized_accept_or_drop_late_detail_lines() {
        let mut app = fixture();
        {
            let world = app.world_mut();
            let mut tracker = world.resource_mut::<QuestTracker>();
            let quest = &mut tracker.active_quests[0];
            quest.status = crate::quest_model::QuestStatus::NotStarted;
            quest.detail.description_lines = (0..40)
                .map(|i| format!("Authoritative server paragraph {i}"))
                .collect();
            world.resource_mut::<QuestUiState>().select_quest(7);
        }
        app.update();
        let world = app.world_mut();
        assert!(!world
            .query::<&QuestUiButton>()
            .iter(world)
            .any(|action| matches!(action, QuestUiButton::AcceptQuest { .. })));
        assert!(world
            .query_filtered::<&Text, With<PhoneQuestText>>()
            .iter(world)
            .any(|text| text.0.contains("paragraph 39")));
    }
}
