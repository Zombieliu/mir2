//! Detail Finish opens an ordinary NPC conversation before submitting the
//! server-offered operation. No rewards or progress are changed locally.
use super::*;
use crate::big_map::BigMapModel;
use crate::entities::EntityKind;
use crate::quest_model::QuestStatus;
use std::time::{Duration, Instant};

const NPC_DATA_RANGE: u32 = 16;
const DIALOG_TIMEOUT: Duration = Duration::from_secs(5);

/// The detail action may coexist with the Diary and ordinary NPC dialogue,
/// but retains every other world-action/modal guard while waiting for a reply.
pub fn quest_turn_in_ui_allows_interaction(state: Option<&NativePlayerUiState>) -> bool {
    let Some(state) = state else { return false; };
    if !matches!(state.core.panel, mir2_ui_core::state::UiPanel::None | mir2_ui_core::state::UiPanel::QuestLog) {
        return false;
    }
    let mut without_quest_panel = state.clone();
    without_quest_panel.core.panel = mir2_ui_core::state::UiPanel::None;
    !without_quest_panel.blocks_world_action(false, false)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct QuestNpcDestination {
    pub object_id: u32,
    pub map_index: i32,
    pub map_title: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
}

impl QuestNpcDestination {
    pub fn label(&self) -> String {
        format!("{} · {} ({},{})", self.map_title, self.name, self.x, self.y)
    }
}

/// Match the server's canonical endpoint lookup: loaded object ID first, with
/// a static-index fallback for legacy imports only. V2 requires loaded IDs.
pub(super) fn destination(quest: &Quest) -> Option<QuestNpcDestination> {
    let loaded_ids = crate::quest_destination::uses_loaded_npc_ids(quest.quest_index);
    let id = match quest.status {
        QuestStatus::ReadyToTurnIn => match quest.finish_npc_index {
            Some(0) if !loaded_ids => quest.accept_npc_index?,
            value => value?,
        },
        QuestStatus::NotStarted => quest.accept_npc_index?,
        _ => return None,
    };
    if id == 0 { return None; }
    static NPCS: std::sync::OnceLock<mir2_game_data::CrystalNpcInfoManifest> = std::sync::OnceLock::new();
    let npcs = &NPCS.get_or_init(mir2_game_data::crystal_npc_info_manifest).npcs;
    let npc = npcs.iter().find(|npc| npc.loaded_object_id == Some(id))
        .or_else(|| (!loaded_ids).then(|| npcs.iter()
            .find(|npc| u32::try_from(npc.npc_index) == Ok(id))).flatten())?;
    let map = mir2_game_data::crystal_respawn_manifest_ref().maps.iter()
        .find(|map| map.map_index == npc.map_index)?;
    Some(QuestNpcDestination {
        object_id: npc.loaded_object_id?, map_index: npc.map_index,
        map_title: map.map_title.clone(), name: npc.name.replace('_', " "),
        x: npc.location.x, y: npc.location.y,
    })
}

pub(super) struct TurnInContext<'a> {
    pub tracker: &'a QuestTracker,
    pub dialog: &'a NpcDialogModel,
    pub map: Option<&'a MapModel>,
    pub entities: Option<&'a EntityModelSet>,
    pub big_map: Option<&'a BigMapModel>,
}

fn in_range(target: &QuestNpcDestination, context: &TurnInContext<'_>) -> bool {
    let (Some(map), Some(entities), Some(big_map)) = (context.map, context.entities, context.big_map) else {
        return false;
    };
    big_map.current_map_index == Some(target.map_index)
        && entities.entities.iter().any(|npc| {
            npc.kind == EntityKind::Npc && npc.object_id.parse::<u32>() == Ok(target.object_id)
                && map.center_x.abs_diff(npc.x).max(map.center_y.abs_diff(npc.y)) <= NPC_DATA_RANGE
        })
}

#[derive(Debug, Clone)]
pub struct PendingQuestTurnIn {
    pub quest_index: i32,
    target: QuestNpcDestination,
    reward: i32,
    map_epoch: u64,
    started: Instant,
}

fn submit(quest_index: i32, reward: i32, state: &mut QuestUiState,
    queue: &mut QuestUiIntentQueue, pending: &mut PendingOperations) {
    state.pending_turn_in = None;
    let intent = QuestUiIntent::FinishQuest { quest_index, selected_item_index: reward };
    if queue.push_pending_intent(pending, intent) {
        state.set_feedback("正在交付任务，等待服务器确认", false);
    } else {
        state.set_feedback("交付请求已在处理中或连接繁忙，请稍后查看任务状态", true);
    }
}

pub(super) fn begin(quest_index: i32, context: &TurnInContext<'_>, state: &mut QuestUiState,
    queue: &mut QuestUiIntentQueue, pending: &mut PendingOperations) {
    if state.pending_turn_in.is_some() { return; }
    let Some(quest) = context.tracker.active_quests.iter().find(|q| q.quest_index == quest_index) else { return; };
    if state.detail_quest_index != Some(quest_index) || !can_finish_quest(quest) {
        state.set_feedback("任务状态已更新，请重新查看详情", true);
        return;
    }
    let reward = state.selected_reward_index.unwrap_or(-1);
    if !is_valid_reward_selection(quest, Some(reward)) {
        state.show_quest_alert(SELECT_REWARD_TEXT);
        return;
    }
    let Some(target) = destination(quest) else {
        state.show_quest_alert("未找到交付 NPC，请按任务说明与交付 NPC 对话。");
        return;
    };
    if !in_range(&target, context) {
        state.show_quest_alert(format!("请先前往 {}，靠近交付 NPC 后点击 Finish。", target.label()));
        return;
    }
    if dialog_exposes_quest_operation(context.dialog, Some(target.object_id), quest_index, true, Some(reward)) {
        submit(quest_index, reward, state, queue, pending);
    } else if queue.push_intent(QuestUiIntent::InteractQuestNpc { quest_index, npc_object_id: target.object_id }) {
        state.set_feedback(format!("正在与 {} 确认交付", target.name), false);
        state.pending_turn_in = Some(PendingQuestTurnIn {
            quest_index, target, reward, map_epoch: context.big_map.unwrap().reset_epoch,
            started: Instant::now(),
        });
    } else {
        state.set_feedback("连接繁忙，请稍后再试", true);
    }
}

fn request_is_current(request: &PendingQuestTurnIn, context: &TurnInContext<'_>, state: &QuestUiState) -> bool {
    let current = context.tracker.active_quests.iter().find(|q| q.quest_index == request.quest_index);
    state.detail_quest_index == Some(request.quest_index)
        && state.selected_reward_index.unwrap_or(-1) == request.reward
        && state.quest_alert_message.is_none() && state.abandon_confirmation_quest_index.is_none()
        && context.big_map.is_some_and(|map| map.reset_epoch == request.map_epoch)
        && current.is_some_and(|quest| can_finish_quest(quest)
            && is_valid_reward_selection(quest, Some(request.reward))
            && destination(quest).as_ref() == Some(&request.target))
        && in_range(&request.target, context)
}

/// Host revalidation of the explicit detail action, without relaxing ordinary
/// world-click/modal guards. A missing/stale pending request cannot authorize it.
pub fn pending_quest_turn_in_allows_interaction(quest_index: i32, npc_object_id: u32,
    state: &QuestUiState, tracker: &QuestTracker, map: &MapModel, entities: &EntityModelSet,
    big_map: &BigMapModel) -> bool {
    state.pending_turn_in.as_ref().is_some_and(|request| {
        request.quest_index == quest_index && request.target.object_id == npc_object_id
            && request.started.elapsed() < DIALOG_TIMEOUT
            && request_is_current(request, &TurnInContext { tracker, dialog: &NpcDialogModel::default(),
                map: Some(map), entities: Some(entities), big_map: Some(big_map) }, state)
    })
}

/// Shared detail-action entry point for hosts using this UI outside its plugin.
pub fn begin_detail_quest_turn_in(quest_index: i32, state: &mut QuestUiState,
    tracker: &QuestTracker, map: &MapModel, entities: &EntityModelSet, big_map: &BigMapModel,
    dialog: &NpcDialogModel, queue: &mut QuestUiIntentQueue, pending: &mut PendingOperations) {
    begin(quest_index, &TurnInContext { tracker, dialog, map: Some(map),
        entities: Some(entities), big_map: Some(big_map) }, state, queue, pending);
}

pub(super) fn advance(context: &TurnInContext<'_>, state: &mut QuestUiState,
    queue: &mut QuestUiIntentQueue, pending: &mut PendingOperations) {
    let Some(request) = state.pending_turn_in.clone() else { return; };
    if !request_is_current(&request, context, state) {
        state.pending_turn_in = None;
        state.set_feedback("交付条件已变化，请重新确认任务与 NPC", true);
    } else if request.started.elapsed() >= DIALOG_TIMEOUT {
        state.pending_turn_in = None;
        state.set_feedback("NPC 尚未确认交付，请重新点击 Finish 或与 NPC 对话", true);
    } else if dialog_exposes_quest_operation(context.dialog, Some(request.target.object_id),
        request.quest_index, true, Some(request.reward)) {
        submit(request.quest_index, request.reward, state, queue, pending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::EntityModel;
    use crate::quest_model::{NpcDialogOption, QuestDetailText};

    fn patrol() -> Quest {
        Quest { quest_index: 2_110_012, accept_npc_index: Some(0), finish_npc_index: Some(24),
            title: "Complete the cave patrol".into(), npc_name: Some("BichonWall Board".into()),
            group: None, min_level_needed: 19, detail: QuestDetailText::default(),
            status: QuestStatus::ReadyToTurnIn, objectives: vec![], rewards: vec![], unknown_text: None }
    }

    fn npc(object_id: u32, x: i32, y: i32) -> EntityModel {
        EntityModel { object_id: object_id.to_string(), kind: EntityKind::Npc,
            name: "NPC".into(), x, y, level: None, direction: None }
    }

    fn offer(object_id: u32) -> NpcDialogModel {
        NpcDialogModel { is_open: true, npc_object_id: Some(object_id),
            options: vec![NpcDialogOption { option_id: "@quest:finish:2110012".into(),
                label: "Finish".into(), enabled: true }], ..default() }
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() });
        app.init_resource::<NativePlayerUiState>();
        app.insert_resource(QuestTracker { active_quests: vec![patrol()] });
        app.insert_resource(QuestGuidance::from_profile_name("newcomer-v2"));
        app.init_resource::<NpcDialogModel>();
        app.init_resource::<NpcDialogNav>();
        app.insert_resource(QuestUiState { detail_quest_index: Some(2_110_012), ..default() });
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.insert_resource(MapModel { center_x: 328, center_y: 261, ..default() });
        app.insert_resource(BigMapModel { current_map_index: Some(1), ..default() });
        app.insert_resource(EntityModelSet { entities: vec![npc(24,334,259), npc(17,371,314)] });
        app.add_systems(Update, process_quest_ui_input);
        app
    }

    fn click_finish(app: &mut App) {
        app.world_mut().spawn((Button, QuestUiButton::PrepareQuestFinish { quest_index: 2_110_012 }, Interaction::Pressed));
        app.update();
    }

    fn drain(app: &mut App) -> Vec<QuestUiIntent> {
        app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents()
    }

    #[test]
    fn quest_turn_in_v2_board_and_growth_claim_use_loaded_id_not_kyle_static_index() {
        let mut quest = patrol();
        for id in [2_110_012, 2_120_020, 2_120_025] {
            quest.quest_index = id;
            let target = destination(&quest).unwrap();
            assert_eq!((target.object_id, target.map_index, target.x, target.y), (24,1,334,259));
            assert!(target.name.contains("Board"));
            assert!(!target.label().contains("Kyle"));
        }
        // Imported q51/q52 also carry canonical loaded IDs: 33 is Alice,
        // whereas the map's static index 33 belongs to Merchant Bull.
        quest.quest_index = 51;
        quest.finish_npc_index = Some(33);
        let legacy = destination(&quest).unwrap();
        assert_eq!(legacy.object_id, 33);
        assert!(legacy.name.contains("Alice"));
        assert!(!legacy.name.contains("Bull"));
    }

    #[test]
    fn quest_turn_in_detail_renders_finish_and_card_locates_board_before_opening_map() {
        let mut world = World::new();
        let quest = patrol();
        let state = QuestUiState::default();
        let guidance = QuestGuidance::from_profile_name("newcomer-v2");
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.spawn_empty().with_children(|parent| render_quest_detail_panel(parent,
            &quest, &guidance, &state, &PendingOperations::default(), None, &Default::default()));
        commands.spawn_empty().with_children(|parent| {
            multi_guidance::render(parent, &QuestTracker { active_quests: vec![quest.clone()] },
                &state, None, &EntityModelSet::default(), &MapModel::default(),
                Some(&BigMapModel { current_map_index: Some(1), ..default() }), "Warrior");
        });
        queue.apply(&mut world);
        assert!(world.query::<&QuestUiButton>().iter(&world).any(|button|
            matches!(button, QuestUiButton::PrepareQuestFinish { quest_index: 2_110_012 })));
        let text = world.query::<&Text>().iter(&world).map(|t| t.0.as_str())
            .collect::<Vec<_>>().join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains("BichonWall Board (334,259)"));
        assert!(!text.contains("Kyle"));
    }

    #[test]
    fn quest_turn_in_detail_click_waits_for_correct_server_offer_and_submits_once() {
        let mut app = app();
        click_finish(&mut app);
        assert_eq!(drain(&mut app), vec![QuestUiIntent::InteractQuestNpc { quest_index: 2_110_012, npc_object_id: 24 }]);
        app.insert_resource(offer(17)); // Wrong NPC cannot authorize the queued click.
        app.update();
        assert!(drain(&mut app).is_empty());
        app.insert_resource(offer(24));
        app.update();
        assert_eq!(drain(&mut app), vec![QuestUiIntent::FinishQuest { quest_index: 2_110_012, selected_item_index: -1 }]);
        app.update();
        assert!(drain(&mut app).is_empty());
        assert!(app.world().resource::<QuestUiState>().pending_turn_in.is_none());
        assert_eq!(app.world().resource::<QuestTracker>().active_quests[0].status, QuestStatus::ReadyToTurnIn);
    }

    #[test]
    fn quest_turn_in_cancels_stale_or_closed_detail_before_server_reply() {
        for scenario in 0..12 {
            let mut app = app();
            click_finish(&mut app);
            drain(&mut app);
            match scenario {
                0 => app.world_mut().resource_mut::<MapModel>().center_x = 500,
                1 => app.world_mut().resource_mut::<BigMapModel>().reset_epoch += 1,
                2 => app.world_mut().resource_mut::<QuestTracker>().active_quests[0].status = QuestStatus::Completed,
                3 => app.world_mut().resource_mut::<EntityModelSet>().entities.clear(),
                4 => app.world_mut().resource_mut::<QuestUiState>().close_detail(),
                5 => app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape),
                6 => app.world_mut().resource_mut::<QuestUiState>().pending_turn_in.as_mut().unwrap().started = Instant::now() - Duration::from_secs(6),
                7 => {
                    let mut model = UiReadModel::default();
                    model.player.hp = 0;
                    model.player.max_hp = 100;
                    app.insert_resource(model);
                }
                8 => {
                    let mut notice = crate::crystal_ui::notice::NoticeDialogState::default();
                    notice.observe(crate::crystal_ui::notice::NoticePacketUpdate { generation: 1, sequence: 1,
                        title: "Notice".into(), message: "Wait".into() });
                    app.insert_resource(notice);
                }
                9 => app.world_mut().resource_mut::<NativePlayerUiState>().core.panel = mir2_ui_core::state::UiPanel::Inventory,
                10 => app.world_mut().resource_mut::<QuestUiState>().selected_reward_index = Some(1),
                _ => app.world_mut().resource_mut::<QuestUiState>().select_quest(2_120_020),
            }
            app.insert_resource(offer(24));
            app.update();
            assert!(!drain(&mut app).iter().any(|intent| matches!(intent, QuestUiIntent::FinishQuest { .. })), "scenario {scenario}");
            assert!(app.world().resource::<QuestUiState>().pending_turn_in.is_none(), "scenario {scenario}");
        }
    }

    #[test]
    fn quest_turn_in_at_kyle_points_back_to_board_without_sending_any_operation() {
        let mut app = app();
        app.world_mut().resource_mut::<MapModel>().center_x = 371;
        app.world_mut().resource_mut::<MapModel>().center_y = 314;
        click_finish(&mut app);
        assert!(drain(&mut app).is_empty());
        assert!(app.world().resource::<QuestUiState>().quest_alert_message.as_ref().unwrap().contains("Board (334,259)"));
    }
}
