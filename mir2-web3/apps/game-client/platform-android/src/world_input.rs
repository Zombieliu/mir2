//! Phone controls are explicit actions, not clicks through an ordinary panel.
//! Reuse the shared modal rules; all panel pointer hit tests remain in Bevy.
use bevy::{ecs::system::SystemParam, prelude::*};
use mir2_client_bevy::{
    crystal_ui::{notice::NoticeDialogState, overlays::NativePlayerUiState},
    quest_model::{NpcDialogModel, QuestTracker},
    quest_ui::QuestUiState,
    read_model::UiReadModel,
};

#[derive(SystemParam)]
pub(crate) struct WorldInputContext<'w> {
    quest: Option<Res<'w, QuestUiState>>,
    tracker: Option<Res<'w, QuestTracker>>,
    dialog: Option<Res<'w, NpcDialogModel>>,
    model: Option<Res<'w, UiReadModel>>,
    notice: Option<Res<'w, NoticeDialogState>>,
}

impl WorldInputContext<'_> {
    pub(crate) fn read_model(&self) -> Option<&UiReadModel> {
        self.model.as_deref()
    }

    /// Presentation only. The independent detail can outlive the diary, but a
    /// stale selection is not a visible window. Keep the existing modal guards.
    pub(crate) fn quest_sidebar_requested(&self, player: &NativePlayerUiState) -> bool {
        !player.storage_open()
            && !player.trade_dialog.open
            && !player.chat_focused()
            && (player.quest_open()
                || self
                    .quest
                    .as_deref()
                    .zip(self.tracker.as_deref())
                    .is_some_and(|(state, tracker)| state.detail_quest(tracker).is_some()))
    }

    pub(crate) fn blocks_views(&self, player: &NativePlayerUiState) -> bool {
        player.blocks_world_action(
            self.dialog.as_deref().is_some_and(|dialog| dialog.is_open),
            false,
        ) || self
            .quest
            .as_deref()
            .is_some_and(QuestUiState::blocks_world_input)
            || self
                .notice
                .as_deref()
                .is_some_and(NoticeDialogState::is_open)
    }

    pub(crate) fn blocks_actions(&self, player: &NativePlayerUiState) -> bool {
        self.blocks_views(player)
            || self
                .model
                .as_deref()
                .is_some_and(|model| model.player.max_hp > 0 && model.player.hp <= 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    use mir2_ui_core::state::{UiPanel, UiScreen};

    fn player(panel: UiPanel) -> NativePlayerUiState {
        let mut player = NativePlayerUiState::default();
        player.core.screen = UiScreen::InGame;
        player.core.panel = panel;
        player
    }

    #[test]
    fn diary_sidebar_does_not_require_optional_models_or_block_world_actions() {
        let mut world = World::new();
        let mut params = SystemState::<WorldInputContext>::new(&mut world);
        let context = params.get(&world).unwrap();
        assert!(context.quest_sidebar_requested(&player(UiPanel::QuestLog)));
        assert!(!context.blocks_views(&player(UiPanel::QuestLog)));
        assert!(!context.blocks_actions(&player(UiPanel::QuestLog)));
        assert!(!context.quest_sidebar_requested(&player(UiPanel::None)));
    }

    #[test]
    fn valid_independent_detail_keeps_sidebar_after_diary_closes() {
        let mut world = World::new();
        let mut state = QuestUiState::default();
        state.select_quest(7);
        world.insert_resource(state);
        let mut params = SystemState::<WorldInputContext>::new(&mut world);
        assert!(!params
            .get(&world)
            .unwrap()
            .quest_sidebar_requested(&player(UiPanel::None)));
        let quest = serde_json::from_value(serde_json::json!({
            "questIndex":7,"title":"OFFLINE layout test","status":"inProgress",
            "objectives":[],"rewards":[]
        }))
        .unwrap();
        world.insert_resource(QuestTracker {
            active_quests: vec![quest],
        });
        let context = params.get(&world).unwrap();
        assert!(context.quest_sidebar_requested(&player(UiPanel::None)));
        assert!(!context.blocks_views(&player(UiPanel::None)));
        world.resource_mut::<QuestTracker>().active_quests.clear();
        assert!(!params
            .get(&world)
            .unwrap()
            .quest_sidebar_requested(&player(UiPanel::None)));
    }

    #[test]
    fn sidebar_never_weakens_npc_modal_or_two_window_transfer_guards() {
        let mut world = World::new();
        let mut dialog = NpcDialogModel::default();
        dialog.is_open = true;
        world.insert_resource(dialog);
        let mut params = SystemState::<WorldInputContext>::new(&mut world);
        let context = params.get(&world).unwrap();
        assert!(context.blocks_views(&player(UiPanel::QuestLog)));
        assert!(context.blocks_actions(&player(UiPanel::QuestLog)));
        assert!(!context.quest_sidebar_requested(&player(UiPanel::Storage)));
        let mut trade = player(UiPanel::QuestLog);
        trade.trade_dialog.open = true;
        assert!(!context.quest_sidebar_requested(&trade));
    }
}
