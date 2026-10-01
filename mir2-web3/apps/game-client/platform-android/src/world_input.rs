//! Phone controls are explicit actions, not clicks through an ordinary panel.
//! Reuse the shared modal rules; all panel pointer hit tests remain in Bevy.
use bevy::{ecs::system::SystemParam, prelude::*};
use mir2_client_bevy::{
    crystal_ui::{notice::NoticeDialogState, overlays::NativePlayerUiState},
    quest_model::NpcDialogModel,
    quest_ui::QuestUiState,
    read_model::UiReadModel,
};

#[derive(SystemParam)]
pub(crate) struct WorldInputContext<'w> {
    quest: Option<Res<'w, QuestUiState>>,
    dialog: Option<Res<'w, NpcDialogModel>>,
    model: Option<Res<'w, UiReadModel>>,
    notice: Option<Res<'w, NoticeDialogState>>,
}

impl WorldInputContext<'_> {
    pub(crate) fn read_model(&self) -> Option<&UiReadModel> {
        self.model.as_deref()
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
