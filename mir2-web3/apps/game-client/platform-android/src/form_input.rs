//! Remaining shared dialog drafts; no inventory, balance or world mutations.
use bevy::{ecs::system::SystemParam, prelude::*};
use mir2_client_bevy::{
    big_map::BigMapModel,
    crystal_ui::overlays::{
        mail_editor::MailLetterEditor,
        BigMapUiState, InventoryDeletePrompt, MailComposeFocus, MailComposeUi, NativePlayerUiState,
    },
    storage::StorageModel,
};

#[derive(SystemParam)]
pub struct FormInput<'w> {
    map: Option<ResMut<'w, BigMapModel>>,
    map_ui: Option<Res<'w, BigMapUiState>>,
    mail_ui: Option<ResMut<'w, MailComposeUi>>,
    mail_editor: Option<ResMut<'w, MailLetterEditor>>,
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
        if let Some(recipient) = self.mail_ui.as_deref().and_then(MailComposeUi::recipient_prompt_draft) {
            return Some(("mail-recipient", recipient, false));
        }
        if let (Some(draft), Some(ui)) = (&state.core.mail_compose, &self.mail_ui) {
            return match ui.focus {
                MailComposeFocus::Message => Some(("mail-message", &draft.message, false)),
                MailComposeFocus::Recipient | MailComposeFocus::Gold => None,
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

    /// Real shaped body caret; absent while the shared layout is stale.
    pub fn mail_caret(&self) -> Option<Vec2> {
        self.mail_editor.as_deref().and_then(MailLetterEditor::ime_caret)
    }

    pub fn mail_body_top(&self) -> f32 {
        match self.mail_ui.as_deref().map(|ui| ui.kind) {
            Some(mir2_client_bevy::crystal_ui::overlays::MailComposeKind::Parcel) => 98.0,
            _ => 92.0,
        }
    }

    pub fn mail_draft_epoch(&self) -> Option<u64> {
        self.mail_ui.as_deref().map(MailComposeUi::draft_epoch)
    }

    pub fn edit(&mut self, state: &mut NativePlayerUiState, field: &str, text: &str) {
        if self.field(state).map(|v| v.0) != Some(field) {
            return;
        }
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
                if let Some(ui) = self.mail_ui.as_deref_mut() {
                    ui.replace_recipient_prompt_draft(text);
                }
            }
            "mail-message" => {
                if let (Some(draft), Some(editor)) = (
                    state.core.mail_compose.as_mut(),
                    self.mail_editor.as_deref_mut(),
                ) {
                    editor.replace_document(draft, text);
                }
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
    fn recipient_ime_cannot_edit_the_read_only_label_in_a_composed_letter() {
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
            ""
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
        world.insert_resource(MailLetterEditor::default());
        let mut mail_ui = MailComposeUi::default();
        mail_ui.focus = MailComposeFocus::Message;
        world.insert_resource(mail_ui);
        let mut state = NativePlayerUiState::default();
        state.core.mail_compose = Some(default());
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", "A\r\nB\rC\0");
        assert_eq!(state.core.mail_compose.as_ref().unwrap().message, "A\nB\nC");
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", &"界".repeat(600));
        assert_eq!(
            state
                .core
                .mail_compose
                .as_ref()
                .unwrap()
                .message
                .chars()
                .count(),
            500
        );
        assert!(state
            .core
            .mail_compose
            .as_ref()
            .unwrap()
            .recipient
            .is_empty());
    }

    #[test]
    fn mail_body_uses_utf16_and_never_splits_an_emoji_grapheme() {
        let mut world = World::new();
        world.insert_resource(MailLetterEditor::default());
        let mut mail_ui = MailComposeUi::default();
        mail_ui.focus = MailComposeFocus::Message;
        world.insert_resource(mail_ui);
        let mut state = NativePlayerUiState::default();
        state.core.mail_compose = Some(default());
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", &"😀".repeat(251));
        let draft = state.core.mail_compose.as_ref().unwrap();
        assert_eq!(draft.message, "😀".repeat(250));
        assert_eq!(draft.message.encode_utf16().count(), 500);
        let editor = world.resource::<MailLetterEditor>().active_editor().unwrap();
        assert_eq!(editor.caret(), draft.message.len());
        assert!(editor.selection().is_empty());
        assert!(editor.is_boundary(editor.caret()));

        system.get_mut(&mut world).unwrap().edit(
            &mut state,
            "mail-message",
            &format!("{}👩‍👩‍👧‍👦tail", "x".repeat(499)),
        );
        assert_eq!(state.core.mail_compose.as_ref().unwrap().message, "x".repeat(499));
        assert!(state.core.mail_compose.as_ref().unwrap().attachment_unique_ids.is_empty());
    }

    #[test]
    fn mail_body_full_replacement_can_clear_the_shared_editor() {
        let mut world = World::new();
        world.insert_resource(MailLetterEditor::default());
        let mut mail_ui = MailComposeUi::default();
        mail_ui.focus = MailComposeFocus::Message;
        world.insert_resource(mail_ui);
        let mut state = NativePlayerUiState::default();
        state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
            recipient: "Receiver".into(),
            message: "old body".into(),
            ..default()
        });
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", "new");
        assert_eq!(state.core.mail_compose.as_ref().unwrap().message, "new");
        world.resource_mut::<MailLetterEditor>().set_composition("preview".into(), None);
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", "");
        let draft = state.core.mail_compose.as_ref().unwrap();
        assert_eq!(draft.recipient, "Receiver");
        assert_eq!(draft.message, "");
        let editor = world.resource::<MailLetterEditor>();
        assert_eq!(editor.active_editor().unwrap().caret(), 0);
        assert_eq!(editor.active_editor().unwrap().text(), "");
        assert!(editor.composition().is_none());
    }

    #[test]
    fn stale_mail_body_event_cannot_edit_a_different_field_or_closed_window() {
        let mut world = World::new();
        world.insert_resource(MailLetterEditor::default());
        let mut mail_ui = MailComposeUi::default();
        mail_ui.focus = MailComposeFocus::Message;
        world.insert_resource(mail_ui);
        let mut state = NativePlayerUiState::default();
        state.core.mail_compose = Some(default());
        let mut system = bevy::ecs::system::SystemState::<FormInput>::new(&mut world);
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", "current");
        world.resource_mut::<MailComposeUi>().focus = MailComposeFocus::Recipient;
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", "stale field");
        assert_eq!(state.core.mail_compose.as_ref().unwrap().message, "current");
        assert_eq!(world.resource::<MailLetterEditor>().active_editor().unwrap().text(), "current");
        state.core.mail_compose = None;
        system
            .get_mut(&mut world)
            .unwrap()
            .edit(&mut state, "mail-message", "stale window");
        assert!(state.core.mail_compose.is_none());
        assert_eq!(world.resource::<MailLetterEditor>().active_editor().unwrap().text(), "current");
    }
}
