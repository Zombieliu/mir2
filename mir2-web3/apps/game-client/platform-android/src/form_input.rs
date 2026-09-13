//! Remaining shared dialog drafts; no inventory, balance or world mutations.
use bevy::{ecs::system::SystemParam, prelude::*};
use mir2_client_bevy::{
    big_map::BigMapModel,
    crystal_ui::overlays::{
        BigMapUiState, InventoryDeletePrompt, MailComposeFocus, MailComposeUi, NativePlayerUiState,
    },
    storage::StorageModel,
};

#[derive(SystemParam)]
pub struct FormInput<'w> {
    map: Option<ResMut<'w, BigMapModel>>,
    map_ui: Option<Res<'w, BigMapUiState>>,
    mail_ui: Option<Res<'w, MailComposeUi>>,
    storage: Option<ResMut<'w, StorageModel>>,
}
impl FormInput<'_> {
    pub fn field<'a>(
        &'a self,
        state: &'a NativePlayerUiState,
    ) -> Option<(&'static str, &'a str, bool)> {
        if let Some(InventoryDeletePrompt::Amount { draft, .. }) = &state.inventory_delete_prompt {
            return Some(("inventory-amount", draft, false));
        }
        if let Some(prompt) = &state.guild_gold_prompt {
            return Some(("guild-amount", &prompt.input.draft, false));
        }
        if let Some(prompt) = &state.trade_dialog.gold_prompt {
            return Some(("trade-amount", &prompt.input.draft, false));
        }
        if state.amount_modal_open() {
            return None;
        }
        if let (Some(draft), Some(ui)) = (&state.core.mail_compose, &self.mail_ui) {
            return match ui.focus {
                MailComposeFocus::Recipient => Some(("mail-recipient", &draft.recipient, false)),
                MailComposeFocus::Message => Some(("mail-message", &draft.message, false)),
                MailComposeFocus::Gold => None,
            };
        }
        if state.bigmap_open() && self.map_ui.as_ref().is_some_and(|ui| ui.search_focused) {
            return self
                .map
                .as_ref()
                .map(|map| ("map-search", map.search.draft.as_str(), false));
        }
        if state.storage_open() {
            if let Some(storage) = &self.storage {
                if storage.has_password && !storage.unlocked {
                    return Some(("storage-password", &storage.password_draft, true));
                }
            }
        }
        None
    }
    pub fn edit(&mut self, state: &mut NativePlayerUiState, field: &str, text: &str) {
        if self.field(state).map(|v| v.0) != Some(field) {
            return;
        }
        use mir2_ui_core::{action::UiAction, reducer::reduce};
        match field {
            "inventory-amount" => {
                if let Some(InventoryDeletePrompt::Amount {
                    draft, select_all, ..
                }) = &mut state.inventory_delete_prompt
                {
                    draft.clear();
                    *select_all = false;
                }
                mir2_client_bevy::crystal_ui::overlays::push_delete_amount_text(state, text);
            }
            "mail-recipient" => {
                state.core = reduce(
                    &state.core,
                    UiAction::SetMailRecipient {
                        recipient: text.into(),
                    },
                )
                .state
            }
            "mail-message" => {
                state.core = reduce(
                    &state.core,
                    UiAction::SetMailMessage {
                        message: text.into(),
                    },
                )
                .state
            }
            "map-search" => {
                if let Some(map) = self.map.as_deref_mut() {
                    map.set_search_draft(text);
                }
            }
            "storage-password" => {
                if let Some(storage) = self.storage.as_deref_mut() {
                    storage.password_draft =
                        text.chars().filter(|c| !c.is_control()).take(16).collect();
                }
            }
            "guild-amount" | "trade-amount" => {
                let input = if field == "guild-amount" {
                    state.guild_gold_prompt.as_mut().map(|p| &mut p.input)
                } else {
                    state
                        .trade_dialog
                        .gold_prompt
                        .as_mut()
                        .map(|p| &mut p.input)
                };
                if let Some(input) = input {
                    input.draft.clear();
                    input.select_all = false;
                    input.push_text(text);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_amount_uses_shared_bounds_without_mutating_the_stack() {
        use mir2_client_bevy::inventory::{InventoryModel, ItemModel};
        let inventory = InventoryModel {
            items: vec![ItemModel {
                unique_id: Some(7),
                container: 0,
                slot: 2,
                quantity: 12,
                key: "fixture".into(),
                name: "Fixture".into(),
                ..default()
            }],
            ..default()
        };
        let mut state = NativePlayerUiState::default();
        assert!(state.open_inventory_delete_for_slot(&inventory, 2));
        let mut world = World::new();
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        let mut forms = system.get_mut(&mut world).unwrap();
        assert_eq!(forms.field(&state).unwrap().0, "inventory-amount");
        forms.edit(&mut state, "inventory-amount", "999abc");
        assert_eq!(forms.field(&state).unwrap().1, "12");
        forms.edit(&mut state, "inventory-amount", "");
        assert_eq!(forms.field(&state).unwrap().1, "");
        state.inventory_delete_prompt = None;
        forms.edit(&mut state, "inventory-amount", "3");
        assert!(state.inventory_delete_prompt.is_none());
        assert_eq!(inventory.items[0].quantity, 12);
    }
    #[test]
    fn locked_storage_keyboard_never_edits_balance_or_unlocks() {
        let mut world = World::new();
        world.insert_resource(StorageModel {
            has_password: true,
            unlocked: false,
            ..default()
        });
        let mut state = NativePlayerUiState::default();
        state.core.screen = mir2_ui_core::state::UiScreen::InGame;
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        {
            let mut forms = system.get_mut(&mut world).unwrap();
            assert_eq!(forms.field(&state).unwrap().2, true);
            forms.edit(&mut state, "storage-password", "12345678901234567890");
            forms.edit(&mut state, "map-search", "stale");
        }
        let storage = world.resource::<StorageModel>();
        assert_eq!(storage.password_draft.len(), 16);
        assert!(!storage.unlocked);
    }
    #[test]
    fn mail_edit_uses_shared_reducer_without_sending_mail() {
        let mut world = World::new();
        world.insert_resource(MailComposeUi::default());
        let mut state = NativePlayerUiState::default();
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft::default());
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-recipient", "UIRecipient");
        assert_eq!(
            state.core.mail_compose.as_ref().unwrap().recipient,
            "UIRecipient"
        );
        assert!(state
            .core
            .mail_compose
            .as_ref()
            .unwrap()
            .attachment_unique_ids
            .is_empty());
    }

    #[test]
    fn mail_body_preserves_newlines_and_shared_length_limit() {
        let mut world = World::new();
        world.insert_resource(MailComposeUi {
            focus: MailComposeFocus::Message,
            ..default()
        });
        let mut state = NativePlayerUiState::default();
        state.core.mail_compose = Some(default());
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", "A\nB");
        assert_eq!(state.core.mail_compose.as_ref().unwrap().message, "A\nB");
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", &"界".repeat(300));
        assert_eq!(
            state
                .core
                .mail_compose
                .as_ref()
                .unwrap()
                .message
                .chars()
                .count(),
            256
        );
        assert!(state
            .core
            .mail_compose
            .as_ref()
            .unwrap()
            .recipient
            .is_empty());
    }
}
