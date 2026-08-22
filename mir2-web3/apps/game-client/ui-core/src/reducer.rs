//! Pure reducer: (state, action) -> (state, effects). No I/O, no Bevy.

use crate::action::UiAction;
use crate::effect::{GatewayCommand, UiEffect};
use crate::state::{UiChatSettings, UiOptions, UiPanel, UiScreen, UiState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub state: UiState,
    pub effects: Vec<UiEffect>,
}

fn in_game(state: &UiState) -> bool {
    state.screen == UiScreen::InGame
}

fn panel(next: &mut UiState, panel: UiPanel) {
    if next.panel == panel {
        next.panel = UiPanel::None;
        next.options_draft = None;
        next.chat_settings_draft = None;
    } else {
        next.options_draft = None;
        next.chat_settings_draft = None;
        next.panel = panel;
        match panel {
            UiPanel::Options => next.options_draft = Some(next.options.clone()),
            UiPanel::ChatSettings => {
                next.chat_settings_draft = Some(next.chat_settings);
            }
            _ => {}
        }
    }
}

fn close_panels(next: &mut UiState) {
    next.panel = UiPanel::None;
    next.options_draft = None;
    next.chat_settings_draft = None;
}

fn draft(next: &mut UiState) -> Option<&mut UiOptions> {
    if next.panel != UiPanel::Options {
        return None;
    }
    if next.options_draft.is_none() {
        next.options_draft = Some(next.options.clone());
    }
    next.options_draft.as_mut()
}

fn chat_draft(next: &mut UiState) -> Option<&mut UiChatSettings> {
    if next.panel != UiPanel::ChatSettings {
        return None;
    }
    if next.chat_settings_draft.is_none() {
        next.chat_settings_draft = Some(next.chat_settings);
    }
    next.chat_settings_draft.as_mut()
}

fn noop(effects: &mut Vec<UiEffect>) {
    effects.push(UiEffect::Noop);
}

pub fn reduce(state: &UiState, action: UiAction) -> Transition {
    let mut next = state.clone();
    let mut effects = Vec::new();
    match action {
        UiAction::Login => {
            if state.screen == UiScreen::Login
                && !state.login_account.is_empty()
                && !state.login_password.is_empty()
            {
                next.screen = UiScreen::Authenticating;
                effects.push(UiEffect::GatewayCommand(GatewayCommand::Login {
                    account: state.login_account.clone(),
                    password: state.login_password.clone(),
                }));
            } else {
                effects.push(UiEffect::ShowNotice {
                    message: "account and password required".into(),
                    is_error: true,
                });
            }
        }
        UiAction::RegisterAccount => {
            if state.screen == UiScreen::Login {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::RegisterAccount {
                    account: state.login_account.clone(),
                    password: state.login_password.clone(),
                }));
            }
        }
        UiAction::ChangePassword => noop(&mut effects),
        UiAction::SafeKey => noop(&mut effects),
        UiAction::CancelLogin => {
            if state.screen == UiScreen::Authenticating {
                next.screen = UiScreen::Login;
            }
        }
        UiAction::RetryConnection => {
            if state.screen == UiScreen::ConnectionLost {
                next.screen = UiScreen::Authenticating;
                effects.push(UiEffect::GatewayCommand(GatewayCommand::RetryConnection));
            }
        }
        UiAction::SelectCharacter { index } => {
            if state.screen == UiScreen::CharacterSelect {
                next.selected_character = Some(index);
            }
        }
        UiAction::StartGame => {
            if state.screen == UiScreen::CharacterSelect {
                if let Some(index) = state.selected_character {
                    next.screen = UiScreen::StartingGame;
                    effects.push(UiEffect::GatewayCommand(GatewayCommand::StartGame {
                        index,
                    }));
                } else {
                    effects.push(UiEffect::ShowNotice {
                        message: "select a character first".into(),
                        is_error: true,
                    });
                }
            }
        }
        UiAction::OpenCharacterCreate => {
            if state.screen == UiScreen::CharacterSelect {
                next.screen = UiScreen::CharacterCreate;
            }
        }
        UiAction::CreateCharacter {
            name,
            class,
            gender,
        } => {
            if state.screen == UiScreen::CharacterCreate {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::CreateCharacter {
                    name,
                    class,
                    gender,
                }));
            }
        }
        UiAction::CancelCharacterCreate => {
            if state.screen == UiScreen::CharacterCreate {
                next.screen = UiScreen::CharacterSelect;
            }
        }
        UiAction::DeleteCharacter { index } => {
            if state.screen == UiScreen::CharacterSelect {
                next.selected_character = Some(index);
                next.panel = UiPanel::DeleteConfirm;
            }
        }
        UiAction::ConfirmDeleteCharacter => {
            if state.screen == UiScreen::CharacterSelect && state.panel == UiPanel::DeleteConfirm {
                if let Some(index) = state.selected_character {
                    next.panel = UiPanel::None;
                    effects.push(UiEffect::GatewayCommand(GatewayCommand::DeleteCharacter {
                        index,
                    }));
                }
            }
        }
        UiAction::CancelDeleteCharacter => {
            if state.panel == UiPanel::DeleteConfirm {
                next.panel = UiPanel::None;
            }
        }
        UiAction::OpenCredits => noop(&mut effects),
        UiAction::ExitApplication => effects.push(UiEffect::ExitApplication),
        UiAction::OpenInventory => {
            if in_game(state) {
                panel(&mut next, UiPanel::Inventory);
            }
        }
        UiAction::OpenCharacter => {
            if in_game(state) {
                panel(&mut next, UiPanel::Character);
            }
        }
        UiAction::OpenSkill => {
            if in_game(state) {
                panel(&mut next, UiPanel::Skill);
            }
        }
        UiAction::OpenQuestLog => {
            if in_game(state) {
                panel(&mut next, UiPanel::QuestLog);
            }
        }
        UiAction::OpenOptions => {
            if in_game(state) {
                panel(&mut next, UiPanel::Options);
            }
        }
        UiAction::OpenMenu => {
            if in_game(state) {
                panel(&mut next, UiPanel::Menu);
            }
        }
        UiAction::OpenGameShop => {
            if in_game(state) {
                panel(&mut next, UiPanel::GameShop);
            }
        }
        UiAction::OpenNpcShop => {
            if in_game(state) {
                panel(&mut next, UiPanel::NpcShop);
            }
        }
        UiAction::OpenMail => {
            if in_game(state) {
                panel(&mut next, UiPanel::Mail);
            }
        }
        UiAction::OpenBigMap => {
            if in_game(state) {
                panel(&mut next, UiPanel::BigMap);
            }
        }
        UiAction::OpenStorage => {
            if in_game(state) {
                panel(&mut next, UiPanel::Storage);
            }
        }
        UiAction::OpenGroup => {
            if in_game(state) {
                panel(&mut next, UiPanel::Group);
            }
        }
        UiAction::OpenGuild => {
            if in_game(state) {
                panel(&mut next, UiPanel::Guild);
            }
        }
        UiAction::OpenTrade => {
            if in_game(state) {
                panel(&mut next, UiPanel::Trade);
            }
        }
        UiAction::OpenMailCompose => {
            if in_game(state) {
                next.panel = UiPanel::Mail;
                next.mail_compose = Some(next.mail_compose.clone().unwrap_or_default());
            }
        }
        UiAction::SetMailRecipient { recipient } => {
            if let Some(compose) = next.mail_compose.as_mut() {
                compose.recipient = recipient.chars().take(32).collect();
            }
        }
        UiAction::SetMailMessage { message } => {
            if let Some(compose) = next.mail_compose.as_mut() {
                compose.message = message.chars().take(256).collect();
            }
        }
        UiAction::SetMailGold { gold } => {
            if let Some(compose) = next.mail_compose.as_mut() {
                compose.gold = gold;
            }
        }
        UiAction::AddMailAttachment { unique_id } => {
            if let Some(compose) = next.mail_compose.as_mut() {
                compose.add_attachment(unique_id);
            }
        }
        UiAction::RemoveMailAttachment { unique_id } => {
            if let Some(compose) = next.mail_compose.as_mut() {
                compose.remove_attachment(unique_id);
            }
        }
        UiAction::SubmitMail => {
            let Some(compose) = state.mail_compose.as_ref() else {
                noop(&mut effects);
                return Transition { state: next, effects };
            };
            let recipient = compose.recipient.trim();
            let message = compose.message.trim();
            if recipient.is_empty() || message.is_empty() {
                effects.push(UiEffect::ShowNotice {
                    message: "recipient and message required".into(),
                    is_error: true,
                });
            } else if compose.attachment_unique_ids.len() > crate::state::MAIL_MAX_ATTACHMENTS
                || compose.attachment_unique_ids.iter().any(|id| *id == 0)
            {
                effects.push(UiEffect::ShowNotice {
                    message: "invalid mail attachment selection".into(),
                    is_error: true,
                });
            } else {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::SendMail {
                    recipient: recipient.to_owned(),
                    message: message.to_owned(),
                    gold: compose.gold,
                    attachment_unique_ids: compose.attachment_unique_ids.clone(),
                }));
            }
        }
        UiAction::CancelMailCompose => {
            next.mail_compose = None;
        }
        UiAction::ToggleMinimap => {
            if in_game(state) {
                next.minimap_visible = !state.minimap_visible;
            }
        }
        UiAction::SetMusicEnabled { enabled } => {
            if let Some(o) = draft(&mut next) {
                o.music_enabled = enabled;
            }
        }
        UiAction::SetMusicVolume { volume } => {
            if let Some(o) = draft(&mut next) {
                o.music_volume = UiOptions::clamp_volume(volume);
            }
        }
        UiAction::SetSoundEnabled { enabled } => {
            if let Some(o) = draft(&mut next) {
                o.sound_enabled = enabled;
            }
        }
        UiAction::SetSoundVolume { volume } => {
            if let Some(o) = draft(&mut next) {
                o.sound_volume = UiOptions::clamp_volume(volume);
            }
        }
        UiAction::SetWindowMode { mode } => {
            if let Some(o) = draft(&mut next) {
                o.window_mode = mode;
            }
        }
        UiAction::ApplyOptions => {
            if state.panel == UiPanel::Options {
                if let Some(options) = next.options_draft.take() {
                    next.options = options.clone();
                    next.panel = UiPanel::None;
                    effects.push(UiEffect::ApplyAudioSettings {
                        music_enabled: options.music_enabled,
                        music_volume: options.music_volume,
                        sound_enabled: options.sound_enabled,
                        sound_volume: options.sound_volume,
                    });
                    effects.push(UiEffect::ApplyWindowMode {
                        mode: options.window_mode,
                    });
                    effects.push(UiEffect::PersistOptions { options });
                }
            }
        }
        UiAction::CancelOptions => {
            if state.panel == UiPanel::Options {
                close_panels(&mut next);
            }
        }
        UiAction::ResetOptionsToDefaults => {
            if let Some(o) = draft(&mut next) {
                *o = UiOptions::default();
            }
        }
        UiAction::SetChatFilterVisibility { channel, visible } => {
            if let Some(settings) = chat_draft(&mut next) {
                settings.set_filter_visible(channel, visible);
            }
        }
        UiAction::SetAllChatFilterVisibility { visible } => {
            if let Some(settings) = chat_draft(&mut next) {
                for channel in crate::state::UiChatChannel::settings() {
                    settings.set_filter_visible(*channel, visible);
                }
            }
        }
        UiAction::SetChatTransparency { transparent } => {
            if let Some(settings) = chat_draft(&mut next) {
                settings.transparent = transparent;
            }
        }
        UiAction::ApplyChatSettings => {
            if state.panel == UiPanel::ChatSettings {
                if let Some(settings) = next.chat_settings_draft.take() {
                    next.chat_settings = settings;
                    next.panel = UiPanel::None;
                    effects.push(UiEffect::ApplyChatSettings { settings });
                    effects.push(UiEffect::PersistChatSettings { settings });
                }
            }
        }
        UiAction::CancelChatSettings | UiAction::CloseChatSettings => {
            if state.panel == UiPanel::ChatSettings {
                close_panels(&mut next);
            }
        }
        UiAction::ResetChatSettingsToDefaults => {
            if let Some(settings) = chat_draft(&mut next) {
                *settings = UiChatSettings::default();
            }
        }
        UiAction::ClosePanel => close_panels(&mut next),
        UiAction::CloseAllPanels => close_panels(&mut next),
        UiAction::GameShopBuy {
            g_index,
            quantity,
            price_type,
        } => {
            if in_game(state)
                && state.is_shop_open()
                && g_index >= 0
                && (1..=99).contains(&quantity)
                && matches!(price_type, 0 | 1)
            {
                if let Some(request) = next.begin_game_shop_purchase(g_index, quantity, price_type) {
                    effects.push(UiEffect::GatewayCommand(GatewayCommand::GameShopBuy {
                        request_id: request.request_id,
                        g_index,
                        quantity,
                        price_type,
                    }));
                }
            }
        }
        UiAction::UseItem { unique_id } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::UseItem {
                    unique_id,
                }));
            }
        }
        UiAction::EquipItem { unique_id, to } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::EquipItem {
                    unique_id,
                    to,
                }));
            }
        }
        UiAction::UnequipItem { unique_id } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::UnequipItem {
                    unique_id,
                }));
            }
        }
        UiAction::DropItem {
            key,
            unique_id,
            count,
            hero_inventory,
        } => {
            if in_game(state) && !key.is_empty() && unique_id != 0 && count != 0 {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::DropItem {
                    key,
                    unique_id,
                    count,
                    hero_inventory,
                }));
            }
        }
        UiAction::MoveItem { grid, from, to } => {
            if in_game(state) && !grid.is_empty() && from >= 0 && to >= 0 && from != to {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::MoveItem {
                    grid,
                    from,
                    to,
                }));
            }
        }
        UiAction::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
        } => {
            if in_game(state)
                && !grid_from.is_empty()
                && !grid_to.is_empty()
                && id_from != 0
                && id_to != 0
                && id_from != id_to
            {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::MergeItem {
                    grid_from,
                    grid_to,
                    id_from,
                    id_to,
                }));
            }
        }
        UiAction::SplitItem {
            unique_id,
            grid,
            count,
        } => {
            if in_game(state) && unique_id != 0 && !grid.is_empty() && count != 0 {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::SplitItem {
                    unique_id,
                    grid,
                    count,
                }));
            }
        }
        UiAction::InteractNpc { object_id } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::InteractNpc {
                    object_id,
                }));
            }
        }
        UiAction::SelectNpcDialog { target } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::SelectNpcDialog {
                    target,
                }));
            }
        }
        UiAction::AcceptQuest {
            npc_index,
            quest_index,
        } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::AcceptQuest {
                    npc_index,
                    quest_index,
                }));
            }
        }
        UiAction::FinishQuest {
            quest_index,
            selected_item_index,
        } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::FinishQuest {
                    quest_index,
                    selected_item_index,
                }));
            }
        }
        UiAction::AbandonQuest { quest_index } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::AbandonQuest {
                    quest_index,
                }));
            }
        }
        UiAction::FocusChat => next.chat_focused = true,
        UiAction::BlurChat => next.chat_focused = false,
        UiAction::SendChat { message } => {
            if in_game(state) && !message.trim().is_empty() {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::SendChat {
                    message,
                }));
            }
        }
        UiAction::SetChatChannel { channel } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::SetChatChannel {
                    channel,
                }));
            }
        }
        UiAction::ScrollChatUp => noop(&mut effects),
        UiAction::ScrollChatDown => noop(&mut effects),
        UiAction::ResizeChat => noop(&mut effects),
        UiAction::OpenChatSettings => {
            if in_game(state) {
                panel(&mut next, UiPanel::ChatSettings);
            }
        }
        UiAction::AttackTarget { object_id } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::AttackTarget {
                    object_id,
                }));
            }
        }
        UiAction::PickUp { object_id } => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::PickUp {
                    object_id,
                }));
            }
        }
        UiAction::TownRevive => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::TownRevive));
            }
        }
        UiAction::Logout => {
            if state.screen == UiScreen::InGame || state.screen == UiScreen::CharacterSelect {
                next.screen = UiScreen::Login;
                next.login_password.clear();
                close_panels(&mut next);
                next.clear_game_shop_session();
                effects.push(UiEffect::GatewayCommand(GatewayCommand::Logout));
            }
        }
        UiAction::ReturnToCharacterSelect => {
            if state.screen == UiScreen::InGame {
                next.screen = UiScreen::CharacterSelect;
                close_panels(&mut next);
            }
        }
    }
    Transition {
        state: next,
        effects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::UiWindowMode;

    fn game() -> UiState {
        UiState {
            screen: UiScreen::InGame,
            ..Default::default()
        }
    }

    #[test]
    fn every_action_variant_is_explicitly_covered() {
        let actions = vec![
            UiAction::Login,
            UiAction::RegisterAccount,
            UiAction::ChangePassword,
            UiAction::SafeKey,
            UiAction::CancelLogin,
            UiAction::RetryConnection,
            UiAction::SelectCharacter { index: 0 },
            UiAction::StartGame,
            UiAction::OpenCharacterCreate,
            UiAction::CreateCharacter {
                name: "n".into(),
                class: "c".into(),
                gender: "g".into(),
            },
            UiAction::CancelCharacterCreate,
            UiAction::DeleteCharacter { index: 0 },
            UiAction::ConfirmDeleteCharacter,
            UiAction::CancelDeleteCharacter,
            UiAction::OpenCredits,
            UiAction::ExitApplication,
            UiAction::OpenInventory,
            UiAction::OpenCharacter,
            UiAction::OpenSkill,
            UiAction::OpenQuestLog,
            UiAction::OpenOptions,
            UiAction::OpenMenu,
            UiAction::OpenGameShop,
            UiAction::OpenNpcShop,
            UiAction::OpenMail,
            UiAction::OpenBigMap,
            UiAction::OpenStorage,
            UiAction::ToggleMinimap,
            UiAction::SetMusicEnabled { enabled: false },
            UiAction::SetMusicVolume { volume: 20 },
            UiAction::SetSoundEnabled { enabled: false },
            UiAction::SetSoundVolume { volume: 20 },
            UiAction::SetWindowMode {
                mode: UiWindowMode::Fullscreen,
            },
            UiAction::ApplyOptions,
            UiAction::CancelOptions,
            UiAction::ResetOptionsToDefaults,
            UiAction::ClosePanel,
            UiAction::CloseAllPanels,
            UiAction::GameShopBuy {
                g_index: 42,
                quantity: 3,
                price_type: 0,
            },
            UiAction::UseItem { unique_id: 1 },
            UiAction::EquipItem {
                unique_id: 1,
                to: 0,
            },
            UiAction::UnequipItem { unique_id: 1 },
            UiAction::DropItem {
                key: "small-hp-drug".into(),
                unique_id: 1,
                count: 1,
                hero_inventory: false,
            },
            UiAction::MoveItem {
                grid: "inventory".into(),
                from: 0,
                to: 1,
            },
            UiAction::MergeItem {
                grid_from: "inventory".into(),
                grid_to: "inventory".into(),
                id_from: 1,
                id_to: 2,
            },
            UiAction::SplitItem {
                unique_id: 1,
                grid: "inventory".into(),
                count: 1,
            },
            UiAction::InteractNpc { object_id: 1 },
            UiAction::SelectNpcDialog { target: "t".into() },
            UiAction::AcceptQuest {
                npc_index: 1,
                quest_index: 1,
            },
            UiAction::FinishQuest {
                quest_index: 1,
                selected_item_index: -1,
            },
            UiAction::AbandonQuest { quest_index: 1 },
            UiAction::FocusChat,
            UiAction::BlurChat,
            UiAction::SendChat {
                message: "m".into(),
            },
            UiAction::SetChatChannel {
                channel: "c".into(),
            },
            UiAction::ScrollChatUp,
            UiAction::ScrollChatDown,
            UiAction::ResizeChat,
            UiAction::OpenChatSettings,
            UiAction::SetChatFilterVisibility {
                channel: crate::state::UiChatChannel::Guild,
                visible: false,
            },
            UiAction::SetChatTransparency { transparent: true },
            UiAction::ApplyChatSettings,
            UiAction::CancelChatSettings,
            UiAction::ResetChatSettingsToDefaults,
            UiAction::CloseChatSettings,
            UiAction::AttackTarget { object_id: 1 },
            UiAction::PickUp { object_id: 1 },
            UiAction::TownRevive,
            UiAction::Logout,
            UiAction::ReturnToCharacterSelect,
        ];
        for action in actions {
            let _ = reduce(&game(), action);
        }
    }

    #[test]
    fn primary_overlay_actions_open_their_own_panels() {
        let state = game();
        let cases = [
            (UiAction::OpenQuestLog, UiPanel::QuestLog),
            (UiAction::OpenOptions, UiPanel::Options),
            (UiAction::OpenMail, UiPanel::Mail),
            (UiAction::OpenGameShop, UiPanel::GameShop),
            (UiAction::OpenNpcShop, UiPanel::NpcShop),
            (UiAction::OpenBigMap, UiPanel::BigMap),
        ];

        for (action, expected_panel) in cases {
            let transition = reduce(&state, action);
            assert_eq!(transition.state.panel, expected_panel);
            assert_ne!(
                transition.state.panel,
                UiPanel::Inventory,
                "primary overlays must not regress into Inventory"
            );
            assert!(
                transition.effects.is_empty(),
                "opening a panel emits no I/O"
            );
        }
    }

    #[test]
    fn game_shop_buy_emits_typed_authoritative_command_and_rejects_bad_input() {
        let mut state = game();
        state.panel = UiPanel::GameShop;
        assert_eq!(
            reduce(
                &state,
                UiAction::GameShopBuy {
                    g_index: 105,
                    quantity: 7,
                    price_type: 1,
                }
            )
            .effects,
            vec![UiEffect::GatewayCommand(GatewayCommand::GameShopBuy {
                request_id: "gs-0000000000000001".into(),
                g_index: 105,
                quantity: 7,
                price_type: 1,
            })]
        );
        for (g_index, quantity, price_type) in [(105, 0, 1), (105, 100, 1), (-1, 1, 1), (105, 1, 2)]
        {
            assert!(reduce(
                &state,
                UiAction::GameShopBuy {
                    g_index,
                    quantity,
                    price_type,
                }
            )
            .effects
            .is_empty());
        }
        state.panel = UiPanel::NpcShop;
        assert!(reduce(
            &state,
            UiAction::GameShopBuy {
                g_index: 105,
                quantity: 1,
                price_type: 1,
            }
        )
        .effects
        .is_empty());
    }

    #[test]
    fn game_shop_request_sequence_exhausts_without_reusing_an_id() {
        let mut state = game();
        state.panel = UiPanel::GameShop;
        state.game_shop_next_request_id = u64::MAX;
        let first = reduce(
            &state,
            UiAction::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            },
        );
        assert_eq!(first.state.game_shop_pending.as_ref().unwrap().request_id,
            "gs-18446744073709551615");
        assert_eq!(first.state.game_shop_next_request_id, 0);
        let second = reduce(
            &first.state,
            UiAction::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            },
        );
        assert!(second.effects.is_empty());
        assert_eq!(second.state.game_shop_pending, first.state.game_shop_pending);
    }

    #[test]
    fn mail_compose_is_typed_bounded_and_emits_only_instance_ids() {
        let mut state = game();
        let opened = reduce(&state, UiAction::OpenMailCompose);
        assert_eq!(opened.state.panel, UiPanel::Mail);
        assert!(opened.state.mail_compose_open());
        state = opened.state;
        state = reduce(
            &state,
            UiAction::SetMailRecipient {
                recipient: "Receiver".into(),
            },
        )
        .state;
        state = reduce(
            &state,
            UiAction::SetMailMessage {
                message: "Hello".into(),
            },
        )
        .state;
        state = reduce(&state, UiAction::AddMailAttachment { unique_id: 77 }).state;
        state = reduce(&state, UiAction::AddMailAttachment { unique_id: 77 }).state;
        state = reduce(&state, UiAction::AddMailAttachment { unique_id: 0 }).state;
        let transition = reduce(&state, UiAction::SubmitMail);
        assert_eq!(
            transition.effects,
            vec![UiEffect::GatewayCommand(GatewayCommand::SendMail {
                recipient: "Receiver".into(),
                message: "Hello".into(),
                gold: 0,
                attachment_unique_ids: vec![77],
            })]
        );
        assert_eq!(
            reduce(&state, UiAction::CancelMailCompose)
                .state
                .mail_compose,
            None
        );
    }

    #[test]
    fn inventory_mutations_preserve_every_authoritative_wire_field() {
        let state = game();
        let cases = [
            (
                UiAction::DropItem {
                    key: "small-hp-drug".into(),
                    unique_id: 7001,
                    count: 3,
                    hero_inventory: false,
                },
                GatewayCommand::DropItem {
                    key: "small-hp-drug".into(),
                    unique_id: 7001,
                    count: 3,
                    hero_inventory: false,
                },
            ),
            (
                UiAction::MoveItem {
                    grid: "inventory".into(),
                    from: 4,
                    to: 9,
                },
                GatewayCommand::MoveItem {
                    grid: "inventory".into(),
                    from: 4,
                    to: 9,
                },
            ),
            (
                UiAction::MergeItem {
                    grid_from: "inventory".into(),
                    grid_to: "inventory".into(),
                    id_from: 7001,
                    id_to: 7002,
                },
                GatewayCommand::MergeItem {
                    grid_from: "inventory".into(),
                    grid_to: "inventory".into(),
                    id_from: 7001,
                    id_to: 7002,
                },
            ),
            (
                UiAction::SplitItem {
                    unique_id: 7001,
                    grid: "inventory".into(),
                    count: 2,
                },
                GatewayCommand::SplitItem {
                    unique_id: 7001,
                    grid: "inventory".into(),
                    count: 2,
                },
            ),
        ];

        for (action, expected) in cases {
            assert_eq!(
                reduce(&state, action).effects,
                vec![UiEffect::GatewayCommand(expected)]
            );
        }
    }

    #[test]
    fn invalid_inventory_mutations_are_effect_free() {
        let state = game();
        for action in [
            UiAction::DropItem {
                key: String::new(),
                unique_id: 7,
                count: 1,
                hero_inventory: false,
            },
            UiAction::MoveItem {
                grid: "inventory".into(),
                from: 3,
                to: 3,
            },
            UiAction::MergeItem {
                grid_from: "inventory".into(),
                grid_to: "inventory".into(),
                id_from: 7,
                id_to: 7,
            },
            UiAction::SplitItem {
                unique_id: 7,
                grid: "inventory".into(),
                count: 0,
            },
        ] {
            assert!(reduce(&state, action).effects.is_empty());
        }
    }

    #[test]
    fn abandon_quest_is_a_dedicated_in_game_command_and_round_trips() {
        let action = UiAction::AbandonQuest { quest_index: 42 };
        let transition = reduce(&game(), action.clone());
        assert_eq!(
            transition.effects,
            vec![UiEffect::GatewayCommand(GatewayCommand::AbandonQuest {
                quest_index: 42,
            })]
        );

        let encoded = serde_json::to_string(&action).expect("serialize abandon action");
        let decoded: UiAction = serde_json::from_str(&encoded).expect("deserialize abandon action");
        assert_eq!(decoded, action);

        let character_select = UiState {
            screen: UiScreen::CharacterSelect,
            ..Default::default()
        };
        assert!(reduce(&character_select, UiAction::AbandonQuest { quest_index: 42 })
            .effects
            .is_empty());
    }

    #[test]
    fn delete_character_requires_confirmation_and_emits_once() {
        let state = UiState {
            screen: UiScreen::CharacterSelect,
            selected_character: Some(2),
            ..Default::default()
        };

        let requested = reduce(&state, UiAction::DeleteCharacter { index: 2 });
        assert_eq!(requested.state.panel, UiPanel::DeleteConfirm);
        assert!(requested.effects.is_empty());

        let cancelled = reduce(&requested.state, UiAction::CancelDeleteCharacter);
        assert_eq!(cancelled.state.panel, UiPanel::None);
        assert!(cancelled.effects.is_empty());
        assert!(reduce(&cancelled.state, UiAction::ConfirmDeleteCharacter)
            .effects
            .is_empty());

        let confirmed = reduce(&requested.state, UiAction::ConfirmDeleteCharacter);
        assert_eq!(confirmed.state.panel, UiPanel::None);
        assert_eq!(
            confirmed.effects,
            vec![UiEffect::GatewayCommand(GatewayCommand::DeleteCharacter {
                index: 2,
            })]
        );
        assert!(reduce(&confirmed.state, UiAction::ConfirmDeleteCharacter)
            .effects
            .is_empty());
    }

    #[test]
    fn options_cancel_and_defaults_keep_committed_values_untouched() {
        let mut state = game();
        state.options.music_volume = 55;
        let opened = reduce(&state, UiAction::OpenOptions).state;
        let edited = reduce(&opened, UiAction::SetMusicVolume { volume: 12 }).state;
        assert_eq!(edited.options.music_volume, 55);
        let reset = reduce(&edited, UiAction::ResetOptionsToDefaults).state;
        assert_eq!(reset.options.music_volume, 55);
        assert_eq!(reset.options_draft.unwrap().music_volume, 80);
        let cancelled = reduce(&edited, UiAction::CancelOptions);
        assert_eq!(cancelled.state.options.music_volume, 55);
        assert_eq!(cancelled.state.panel, UiPanel::None);
    }

    #[test]
    fn options_apply_commits_and_emits_typed_runtime_and_persistence_effects() {
        let opened = reduce(&game(), UiAction::OpenOptions).state;
        let edited = reduce(
            &opened,
            UiAction::SetWindowMode {
                mode: UiWindowMode::Fullscreen,
            },
        )
        .state;
        let applied = reduce(&edited, UiAction::ApplyOptions);
        assert_eq!(applied.state.options.window_mode, UiWindowMode::Fullscreen);
        assert_eq!(applied.state.panel, UiPanel::None);
        assert_eq!(applied.effects.len(), 3);
        assert!(applied
            .effects
            .iter()
            .any(|effect| matches!(effect, UiEffect::ApplyAudioSettings { .. })));
        assert!(applied.effects.iter().any(|effect| matches!(
            effect,
            UiEffect::ApplyWindowMode {
                mode: UiWindowMode::Fullscreen
            }
        )));
        assert!(applied
            .effects
            .iter()
            .any(|effect| matches!(effect, UiEffect::PersistOptions { .. })));
        assert!(
            reduce(&applied.state, UiAction::ApplyOptions)
                .effects
                .is_empty(),
            "the successor state must not replay option side effects"
        );
    }

    #[test]
    fn chat_settings_stage_visibility_and_transparency_until_apply() {
        let opened = reduce(&game(), UiAction::OpenChatSettings).state;
        assert_eq!(opened.panel, UiPanel::ChatSettings);
        assert_eq!(opened.chat_settings, UiChatSettings::default());

        let edited = reduce(
            &reduce(
                &opened,
                UiAction::SetChatFilterVisibility {
                    channel: crate::state::UiChatChannel::Guild,
                    visible: false,
                },
            )
            .state,
            UiAction::SetChatTransparency { transparent: true },
        )
        .state;
        assert!(!edited.chat_settings.filter_guild);
        assert_eq!(edited.chat_settings_draft.unwrap().filter_guild, true);
        assert!(edited.chat_settings_draft.unwrap().transparent);

        let cancelled = reduce(&edited, UiAction::CancelChatSettings);
        assert_eq!(cancelled.state.panel, UiPanel::None);
        assert_eq!(cancelled.state.chat_settings, UiChatSettings::default());
        assert!(cancelled.state.chat_settings_draft.is_none());

        let opened_again = reduce(&game(), UiAction::OpenChatSettings).state;
        let changed = reduce(
            &opened_again,
            UiAction::SetChatFilterVisibility {
                channel: crate::state::UiChatChannel::Guild,
                visible: false,
            },
        )
        .state;
        let applied = reduce(&changed, UiAction::ApplyChatSettings);
        assert_eq!(applied.state.panel, UiPanel::None);
        assert!(applied.state.chat_settings.filter_guild);
        assert_eq!(applied.effects.len(), 2);
        assert!(applied
            .effects
            .iter()
            .any(|effect| matches!(effect, UiEffect::ApplyChatSettings { settings } if settings.filter_guild)));
        assert!(applied.effects.iter().any(
            |effect| matches!(effect, UiEffect::PersistChatSettings { settings } if settings.filter_guild)
        ));
        assert!(
            reduce(&applied.state, UiAction::ApplyChatSettings)
                .effects
                .is_empty(),
            "the successor state must not replay chat setting side effects"
        );
    }

    #[test]
    fn chat_settings_defaults_only_replace_the_draft() {
        let mut state = game();
        state.chat_settings.filter_group = true;
        let opened = reduce(&state, UiAction::OpenChatSettings).state;
        let changed = reduce(&opened, UiAction::ResetChatSettingsToDefaults);
        assert!(changed.state.chat_settings.filter_group);
        assert_eq!(
            changed.state.chat_settings_draft,
            Some(UiChatSettings::default())
        );
    }

    #[test]
    fn crystal_dialog_toggle_all_does_not_change_trade_filter() {
        let mut state = game();
        state.chat_settings.filter_trade = true;
        let opened = reduce(&state, UiAction::OpenChatSettings).state;
        let changed = reduce(
            &opened,
            UiAction::SetAllChatFilterVisibility { visible: false },
        )
        .state;
        let draft = changed.chat_settings_draft.unwrap();
        assert!(draft.filter_normal);
        assert!(draft.filter_guild);
        assert!(
            draft.filter_trade,
            "the source dialog has no Trade checkbox"
        );
    }
}
