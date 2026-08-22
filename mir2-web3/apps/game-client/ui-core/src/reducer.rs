//! Pure reducer: (state, action) -> (state, effects). No I/O, no Bevy.

use crate::action::UiAction;
use crate::effect::{
    valid_guild_storage_gold_change, valid_guild_storage_item_change, GatewayCommand,
    SecurityRequest, UiEffect,
};
use crate::state::{
    UiChatSettings, UiOptions, UiPanel, UiPlatformSettings, UiScreen, UiSecurityPanel, UiState,
};
use crate::storage::StorageOperation;

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
        next.platform_settings_draft = None;
        next.chat_settings_draft = None;
    } else {
        next.options_draft = None;
        next.platform_settings_draft = None;
        next.chat_settings_draft = None;
        next.panel = panel;
        match panel {
            // Crystal OptionDialog changes Settings immediately. There is no
            // draft to commit or discard when it closes.
            UiPanel::Options => {}
            UiPanel::PlatformSettings => {
                next.platform_settings_draft = Some(next.platform_settings);
            }
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
    next.platform_settings_draft = None;
    next.chat_settings_draft = None;
}

fn platform_draft(next: &mut UiState) -> Option<&mut UiPlatformSettings> {
    if next.panel != UiPanel::PlatformSettings {
        return None;
    }
    if next.platform_settings_draft.is_none() {
        next.platform_settings_draft = Some(next.platform_settings);
    }
    next.platform_settings_draft.as_mut()
}

fn persist_crystal_options(effects: &mut Vec<UiEffect>, options: UiOptions, apply_audio: bool) {
    if apply_audio {
        effects.push(UiEffect::ApplyAudioSettings {
            music_enabled: options.music_enabled,
            music_volume: options.music_volume,
            sound_enabled: options.sound_enabled,
            sound_volume: options.sound_volume,
        });
    }
    effects.push(UiEffect::PersistOptions { options });
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

fn security_notice(effects: &mut Vec<UiEffect>, message: &str) {
    effects.push(UiEffect::ShowNotice {
        message: message.to_owned(),
        is_error: true,
    });
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
        UiAction::ChangePassword => {
            if state.screen == UiScreen::Login {
                next.security.panel = UiSecurityPanel::ChangePassword;
                next.security.change_password_pending = false;
            }
        }
        UiAction::SafeKey => {
            if state.screen == UiScreen::Login {
                next.security.panel = UiSecurityPanel::SafeKey;
                next.security.change_password_pending = false;
            }
        }
        UiAction::SubmitChangePassword {
            account,
            old_password,
            new_password,
            confirm_password,
        } => {
            if state.screen != UiScreen::Login
                || state.security.panel != UiSecurityPanel::ChangePassword
            {
                return Transition {
                    state: next,
                    effects,
                };
            }
            if state.security.change_password_pending {
                return Transition {
                    state: next,
                    effects,
                };
            }
            if account.trim().is_empty() {
                security_notice(&mut effects, "account is required");
            } else if old_password.is_empty() || new_password.is_empty() {
                security_notice(&mut effects, "password fields are required");
            } else if new_password != confirm_password {
                security_notice(&mut effects, "new passwords do not match");
            } else {
                next.security.change_password_pending = true;
                effects.push(UiEffect::SecurityRequest(SecurityRequest::ChangePassword {
                    account,
                    old_password,
                    new_password,
                }));
            }
        }
        UiAction::ChangePasswordResult { success, message } => {
            if !state.security.change_password_pending {
                return Transition {
                    state: next,
                    effects,
                };
            }
            next.security.change_password_pending = false;
            if success {
                next.security.panel = UiSecurityPanel::None;
            }
            effects.push(UiEffect::ShowNotice {
                message,
                is_error: !success,
            });
        }
        UiAction::CancelChangePassword => {
            if state.security.panel == UiSecurityPanel::ChangePassword {
                next.security.panel = UiSecurityPanel::None;
            }
        }
        UiAction::CloseSafeKey => {
            if state.security.panel == UiSecurityPanel::SafeKey {
                next.security.panel = UiSecurityPanel::None;
            }
        }
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
        UiAction::RequestGuildStorage => {
            if in_game(state) {
                effects.push(UiEffect::GatewayCommand(GatewayCommand::GuildStorageList));
            }
        }
        UiAction::GuildStorageGoldChange {
            change_type,
            amount,
        } => {
            if in_game(state) && valid_guild_storage_gold_change(change_type, amount) {
                effects.push(UiEffect::GatewayCommand(
                    GatewayCommand::GuildStorageGoldChange {
                        change_type,
                        amount,
                    },
                ));
            }
        }
        UiAction::GuildStorageItemChange {
            change_type,
            from,
            to,
        } => {
            if in_game(state) && valid_guild_storage_item_change(change_type, from, to) {
                if change_type == crate::effect::GUILD_STORAGE_LIST_CHANGE_TYPE {
                    effects.push(UiEffect::GatewayCommand(GatewayCommand::GuildStorageList));
                } else {
                    effects.push(UiEffect::GatewayCommand(
                        GatewayCommand::GuildStorageItemChange {
                            change_type,
                            from,
                            to,
                        },
                    ));
                }
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
                return Transition {
                    state: next,
                    effects,
                };
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
            if state.panel == UiPanel::Options && state.options.music_enabled != enabled {
                next.options.music_enabled = enabled;
                persist_crystal_options(&mut effects, next.options.clone(), true);
            }
        }
        UiAction::SetMusicVolume { volume } => {
            let volume = UiOptions::clamp_volume(volume);
            if state.panel == UiPanel::Options && state.options.music_volume != volume {
                next.options.music_volume = volume;
                persist_crystal_options(&mut effects, next.options.clone(), true);
            }
        }
        UiAction::SetSoundEnabled { enabled } => {
            if state.panel == UiPanel::Options && state.options.sound_enabled != enabled {
                next.options.sound_enabled = enabled;
                persist_crystal_options(&mut effects, next.options.clone(), true);
            }
        }
        UiAction::SetSoundVolume { volume } => {
            let volume = UiOptions::clamp_volume(volume);
            if state.panel == UiPanel::Options && state.options.sound_volume != volume {
                next.options.sound_volume = volume;
                persist_crystal_options(&mut effects, next.options.clone(), true);
            }
        }
        UiAction::SetCrystalOption { option, enabled } => {
            if state.panel == UiPanel::Options && state.options.crystal_option(option) != enabled {
                next.options.set_crystal_option(option, enabled);
                persist_crystal_options(&mut effects, next.options.clone(), false);
            }
        }
        UiAction::RequestObserve { allow } => {
            if state.panel == UiPanel::Options
                && state.observe_allowed != allow
                && state.observe_request_pending.is_none()
            {
                next.observe_request_pending = Some(allow);
                effects.push(UiEffect::RequestObserve { allow });
            }
        }
        UiAction::ObserveAuthoritativeChanged { allow } => {
            next.observe_allowed = allow;
            next.observe_request_pending = None;
        }
        UiAction::OpenPlatformSettings => {
            if in_game(state) {
                panel(&mut next, UiPanel::PlatformSettings);
            }
        }
        UiAction::SetPlatformWindowMode { mode } => {
            if let Some(settings) = platform_draft(&mut next) {
                settings.window_mode = mode;
            }
        }
        UiAction::ApplyPlatformSettings => {
            if state.panel == UiPanel::PlatformSettings {
                if let Some(settings) = next.platform_settings_draft.take() {
                    next.platform_settings = settings;
                    // Keep old renderers compiling without allowing Crystal
                    // controls to mutate the platform setting.
                    next.options.window_mode = settings.window_mode;
                    next.panel = UiPanel::None;
                    effects.push(UiEffect::ApplyWindowMode {
                        mode: settings.window_mode,
                    });
                    effects.push(UiEffect::PersistOptions {
                        options: next.options.clone(),
                    });
                }
            }
        }
        UiAction::CancelPlatformSettings => {
            if state.panel == UiPanel::PlatformSettings {
                close_panels(&mut next);
            }
        }
        UiAction::ResetPlatformSettingsToDefaults => {
            if let Some(settings) = platform_draft(&mut next) {
                *settings = UiPlatformSettings::default();
            }
        }
        // These names remain only so an older renderer can compile while its
        // non-Crystal settings surface is migrated. They are deliberately
        // inert from Crystal's Options panel: Close is Hide, never Apply.
        UiAction::SetWindowMode { .. }
        | UiAction::ApplyOptions
        | UiAction::CancelOptions
        | UiAction::ResetOptionsToDefaults => {}
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
                if let Some(request) = next.begin_game_shop_purchase(g_index, quantity, price_type)
                {
                    effects.push(UiEffect::GatewayCommand(GatewayCommand::GameShopBuy {
                        request_id: request.request_id,
                        g_index,
                        quantity,
                        price_type,
                    }));
                }
            }
        }
        UiAction::StoreItem { from, to } => {
            if in_game(state) {
                if let Some(request) =
                    next.begin_storage_request(StorageOperation::StoreItem, from, to)
                {
                    effects.push(UiEffect::GatewayCommand(GatewayCommand::StoreItem {
                        request_id: request.request_id,
                        from,
                        to,
                    }));
                }
            }
        }
        UiAction::TakeBackItem { from, to } => {
            if in_game(state) {
                if let Some(request) =
                    next.begin_storage_request(StorageOperation::TakeBackItem, from, to)
                {
                    effects.push(UiEffect::GatewayCommand(GatewayCommand::TakeBackItem {
                        request_id: request.request_id,
                        from,
                        to,
                    }));
                }
            }
        }
        UiAction::StorageReceiptReceived { receipt } => {
            next.apply_storage_receipt(receipt);
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
                next.clear_storage_session();
                effects.push(UiEffect::GatewayCommand(GatewayCommand::Logout));
            }
        }
        UiAction::ReturnToCharacterSelect => {
            if state.screen == UiScreen::InGame {
                next.screen = UiScreen::CharacterSelect;
                close_panels(&mut next);
                next.clear_storage_session();
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
            UiAction::SubmitChangePassword {
                account: "account".into(),
                old_password: crate::effect::SecretText::new("old"),
                new_password: crate::effect::SecretText::new("new"),
                confirm_password: crate::effect::SecretText::new("new"),
            },
            UiAction::ChangePasswordResult {
                success: false,
                message: "rejected".into(),
            },
            UiAction::CancelChangePassword,
            UiAction::CloseSafeKey,
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
            UiAction::OpenGroup,
            UiAction::OpenGuild,
            UiAction::RequestGuildStorage,
            UiAction::GuildStorageGoldChange {
                change_type: 0,
                amount: 1,
            },
            UiAction::GuildStorageItemChange {
                change_type: 2,
                from: 0,
                to: 1,
            },
            UiAction::ToggleMinimap,
            UiAction::SetMusicEnabled { enabled: false },
            UiAction::SetMusicVolume { volume: 20 },
            UiAction::SetSoundEnabled { enabled: false },
            UiAction::SetSoundVolume { volume: 20 },
            UiAction::SetCrystalOption {
                option: crate::state::UiCrystalOption::Effect,
                enabled: false,
            },
            UiAction::RequestObserve { allow: true },
            UiAction::ObserveAuthoritativeChanged { allow: true },
            UiAction::OpenPlatformSettings,
            UiAction::SetPlatformWindowMode {
                mode: UiWindowMode::Fullscreen,
            },
            UiAction::ApplyPlatformSettings,
            UiAction::CancelPlatformSettings,
            UiAction::ResetPlatformSettingsToDefaults,
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
            UiAction::StoreItem { from: 1, to: 2 },
            UiAction::TakeBackItem { from: 2, to: 1 },
            UiAction::StorageReceiptReceived {
                receipt: crate::storage::StorageReceipt {
                    protocol: crate::storage::NATIVE_STORAGE_RECEIPT_PROTOCOL.into(),
                    request_id: "st-1".into(),
                    operation: StorageOperation::StoreItem,
                    from: 1,
                    to: 2,
                    success: true,
                },
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
    fn guild_storage_actions_emit_shared_typed_commands_and_enforce_bounds() {
        let state = game();
        assert_eq!(
            reduce(&state, UiAction::RequestGuildStorage).effects,
            vec![UiEffect::GatewayCommand(GatewayCommand::GuildStorageList)]
        );
        assert_eq!(
            reduce(
                &state,
                UiAction::GuildStorageGoldChange {
                    change_type: 1,
                    amount: 250,
                }
            )
            .effects,
            vec![UiEffect::GatewayCommand(
                GatewayCommand::GuildStorageGoldChange {
                    change_type: 1,
                    amount: 250,
                }
            )]
        );
        assert_eq!(
            reduce(
                &state,
                UiAction::GuildStorageItemChange {
                    change_type: 2,
                    from: 4,
                    to: 7,
                }
            )
            .effects,
            vec![UiEffect::GatewayCommand(
                GatewayCommand::GuildStorageItemChange {
                    change_type: 2,
                    from: 4,
                    to: 7,
                }
            )]
        );
        assert_eq!(
            reduce(
                &state,
                UiAction::GuildStorageItemChange {
                    change_type: crate::effect::GUILD_STORAGE_LIST_CHANGE_TYPE,
                    from: 0,
                    to: 0,
                }
            )
            .effects,
            vec![UiEffect::GatewayCommand(GatewayCommand::GuildStorageList)]
        );

        for action in [
            UiAction::GuildStorageGoldChange {
                change_type: 2,
                amount: 1,
            },
            UiAction::GuildStorageGoldChange {
                change_type: 0,
                amount: 0,
            },
            UiAction::GuildStorageItemChange {
                change_type: 2,
                from: -1,
                to: 0,
            },
            UiAction::GuildStorageItemChange {
                change_type: 2,
                from: 0,
                to: crate::effect::GUILD_STORAGE_SLOT_COUNT,
            },
            UiAction::GuildStorageItemChange {
                change_type: crate::effect::GUILD_STORAGE_LIST_CHANGE_TYPE,
                from: 1,
                to: 0,
            },
        ] {
            assert!(
                reduce(&state, action).effects.is_empty(),
                "invalid guild storage input must not reach the gateway"
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
        assert_eq!(
            first.state.game_shop_pending.as_ref().unwrap().request_id,
            "gs-18446744073709551615"
        );
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
        assert_eq!(
            second.state.game_shop_pending,
            first.state.game_shop_pending
        );
    }

    #[test]
    fn storage_reducer_allocates_exact_request_and_rejects_old_receipts() {
        let state = game();
        let first = reduce(&state, UiAction::StoreItem { from: 3, to: 9 });
        assert_eq!(
            first.state.storage_pending.as_ref().unwrap().request_id,
            "st-0000000000000001"
        );
        assert_eq!(first.state.storage_next_request_id, 2);
        assert_eq!(
            first.effects,
            vec![UiEffect::GatewayCommand(GatewayCommand::StoreItem {
                request_id: "st-0000000000000001".into(),
                from: 3,
                to: 9,
            })]
        );

        // A second personal-storage operation cannot overtake the first.
        let duplicate = reduce(&first.state, UiAction::TakeBackItem { from: 9, to: 3 });
        assert!(duplicate.effects.is_empty());
        assert_eq!(duplicate.state.storage_pending, first.state.storage_pending);

        let old_receipt = crate::storage::StorageReceipt {
            protocol: crate::storage::NATIVE_STORAGE_RECEIPT_PROTOCOL.into(),
            request_id: "st-0000000000000000".into(),
            operation: StorageOperation::StoreItem,
            from: 3,
            to: 9,
            success: true,
        };
        let unchanged = reduce(
            &first.state,
            UiAction::StorageReceiptReceived {
                receipt: old_receipt,
            },
        );
        assert_eq!(unchanged.state.storage_pending, first.state.storage_pending);

        let exact = crate::storage::StorageReceipt {
            protocol: crate::storage::NATIVE_STORAGE_RECEIPT_PROTOCOL.into(),
            request_id: "st-0000000000000001".into(),
            operation: StorageOperation::StoreItem,
            from: 3,
            to: 9,
            success: true,
        };
        let applied = reduce(
            &first.state,
            UiAction::StorageReceiptReceived { receipt: exact },
        );
        assert!(applied.state.storage_pending.is_none());
        assert!(applied.state.storage_last_receipt.is_some());
    }

    #[test]
    fn storage_request_sequence_fails_closed_after_u64_max() {
        let mut state = game();
        state.storage_next_request_id = u64::MAX;
        let first = reduce(&state, UiAction::StoreItem { from: 1, to: 2 });
        assert_eq!(
            first.state.storage_pending.as_ref().unwrap().request_id,
            "st-18446744073709551615"
        );
        assert_eq!(first.state.storage_next_request_id, 0);
        let second = reduce(
            &first.state,
            UiAction::StorageReceiptReceived {
                receipt: crate::storage::StorageReceipt {
                    protocol: crate::storage::NATIVE_STORAGE_RECEIPT_PROTOCOL.into(),
                    request_id: "st-18446744073709551615".into(),
                    operation: StorageOperation::StoreItem,
                    from: 1,
                    to: 2,
                    success: true,
                },
            },
        );
        let third = reduce(&second.state, UiAction::StoreItem { from: 2, to: 3 });
        assert!(third.effects.is_empty());
        assert_eq!(third.state.storage_next_request_id, 0);
    }

    #[test]
    fn storage_session_clear_drops_pending_without_reusing_request_ids() {
        let first = reduce(&game(), UiAction::StoreItem { from: 3, to: 9 });
        let logged_out = reduce(&first.state, UiAction::Logout);
        assert!(logged_out.state.storage_pending.is_none());
        assert!(logged_out.state.storage_unknown);
        assert_eq!(logged_out.state.storage_next_request_id, 2);

        let mut next_session = logged_out.state;
        next_session.screen = UiScreen::InGame;
        let second = reduce(&next_session, UiAction::TakeBackItem { from: 9, to: 3 });
        assert_eq!(
            second.state.storage_pending.as_ref().unwrap().request_id,
            "st-0000000000000002"
        );
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
        assert!(reduce(
            &character_select,
            UiAction::AbandonQuest { quest_index: 42 }
        )
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
    fn crystal_options_commit_immediately_and_close_never_rolls_them_back() {
        let mut state = game();
        state.options.music_volume = 55;
        let opened = reduce(&state, UiAction::OpenOptions).state;
        assert!(opened.options_draft.is_none());

        let volume = reduce(&opened, UiAction::SetMusicVolume { volume: 12 });
        assert_eq!(volume.state.options.music_volume, 12);
        assert!(matches!(
            volume.effects.as_slice(),
            [
                UiEffect::ApplyAudioSettings {
                    music_volume: 12,
                    ..
                },
                UiEffect::PersistOptions { .. }
            ]
        ));
        let edited = reduce(
            &volume.state,
            UiAction::SetCrystalOption {
                option: crate::state::UiCrystalOption::Effect,
                enabled: false,
            },
        );
        assert_eq!(edited.state.options.music_volume, 12);
        assert!(!edited.state.options.effect);
        assert!(matches!(
            edited.effects.as_slice(),
            [UiEffect::PersistOptions { options }] if !options.effect
        ));

        // Crystal's CloseButton calls Hide(), not Apply/Cancel. Closing must
        // retain settings that have already been written.
        let closed = reduce(&edited.state, UiAction::ClosePanel);
        assert_eq!(closed.state.options.music_volume, 12);
        assert!(!closed.state.options.effect);
        assert_eq!(closed.state.panel, UiPanel::None);
        assert!(closed.state.options_draft.is_none());
        assert!(closed.effects.is_empty());
    }

    #[test]
    fn every_crystal_switch_persists_at_click_time() {
        let opened = reduce(&game(), UiAction::OpenOptions).state;
        let mut edited = opened;
        for option in [
            crate::state::UiCrystalOption::SkillMode,
            crate::state::UiCrystalOption::SkillBar,
            crate::state::UiCrystalOption::Effect,
            crate::state::UiCrystalOption::DropView,
            crate::state::UiCrystalOption::NameView,
            crate::state::UiCrystalOption::HpView,
            crate::state::UiCrystalOption::NewMove,
        ] {
            let value = !edited.options.crystal_option(option);
            let transition = reduce(
                &edited,
                UiAction::SetCrystalOption {
                    option,
                    enabled: value,
                },
            );
            assert!(matches!(
                transition.effects.as_slice(),
                [UiEffect::PersistOptions { options }]
                    if options.crystal_option(option) == value
            ));
            edited = transition.state;
        }
        assert!(edited.options.skill_mode);
        assert!(!edited.options.skill_bar);
        assert!(!edited.options.effect);
        assert!(!edited.options.drop_view);
        assert!(!edited.options.name_view);
        assert!(!edited.options.hp_view);
        assert!(edited.options.new_move);
    }

    #[test]
    fn platform_settings_are_the_only_staged_window_extension() {
        let opened = reduce(&game(), UiAction::OpenPlatformSettings).state;
        assert_eq!(opened.panel, UiPanel::PlatformSettings);
        assert_eq!(
            opened.platform_settings_draft,
            Some(opened.platform_settings)
        );

        let staged = reduce(
            &opened,
            UiAction::SetPlatformWindowMode {
                mode: UiWindowMode::Fullscreen,
            },
        )
        .state;
        assert_eq!(staged.platform_settings.window_mode, UiWindowMode::Windowed);
        assert_eq!(
            staged
                .platform_settings_draft
                .map(|settings| settings.window_mode),
            Some(UiWindowMode::Fullscreen)
        );
        let cancelled = reduce(&staged, UiAction::CancelPlatformSettings);
        assert_eq!(
            cancelled.state.platform_settings.window_mode,
            UiWindowMode::Windowed
        );
        assert_eq!(cancelled.state.panel, UiPanel::None);

        let reopened = reduce(&cancelled.state, UiAction::OpenPlatformSettings).state;
        let applied = reduce(
            &reduce(
                &reopened,
                UiAction::SetPlatformWindowMode {
                    mode: UiWindowMode::Fullscreen,
                },
            )
            .state,
            UiAction::ApplyPlatformSettings,
        );
        assert_eq!(
            applied.state.platform_settings.window_mode,
            UiWindowMode::Fullscreen
        );
        assert_eq!(applied.state.options.window_mode, UiWindowMode::Fullscreen);
        assert!(matches!(
            applied.effects.as_slice(),
            [UiEffect::ApplyWindowMode { mode: UiWindowMode::Fullscreen }, UiEffect::PersistOptions { options }]
                if options.window_mode == UiWindowMode::Fullscreen
        ));
    }

    #[test]
    fn observe_is_request_only_until_authoritative_state_arrives() {
        let opened = reduce(&game(), UiAction::OpenOptions).state;
        let requested = reduce(&opened, UiAction::RequestObserve { allow: true });
        assert!(!requested.state.observe_allowed);
        assert_eq!(requested.state.observe_request_pending, Some(true));
        assert_eq!(
            requested.effects,
            vec![UiEffect::RequestObserve { allow: true }]
        );

        let duplicate = reduce(&requested.state, UiAction::RequestObserve { allow: true });
        assert!(duplicate.effects.is_empty());
        assert!(!duplicate.state.observe_allowed);

        let authoritative = reduce(
            &requested.state,
            UiAction::ObserveAuthoritativeChanged { allow: true },
        );
        assert!(authoritative.state.observe_allowed);
        assert_eq!(authoritative.state.observe_request_pending, None);
        assert!(authoritative.effects.is_empty());

        let already_equal = reduce(
            &authoritative.state,
            UiAction::RequestObserve { allow: true },
        );
        assert!(already_equal.effects.is_empty());
    }

    #[test]
    fn security_flows_are_explicit_and_never_stage_credentials() {
        let login = UiState {
            screen: UiScreen::Login,
            login_account: "account".into(),
            ..Default::default()
        };

        let opened = reduce(&login, UiAction::ChangePassword);
        assert_eq!(
            opened.state.security.panel,
            crate::state::UiSecurityPanel::ChangePassword
        );
        assert!(opened.effects.is_empty());

        let request = reduce(
            &opened.state,
            UiAction::SubmitChangePassword {
                account: "account".into(),
                old_password: crate::effect::SecretText::new("old-secret"),
                new_password: crate::effect::SecretText::new("new-secret"),
                confirm_password: crate::effect::SecretText::new("new-secret"),
            },
        );
        assert!(request.state.security.change_password_pending);
        assert!(request.state.login_password.is_empty());
        assert!(matches!(
            &request.effects[..],
            [UiEffect::SecurityRequest(SecurityRequest::ChangePassword {
                account,
                old_password,
                new_password,
            })] if account == "account"
                && old_password.as_str() == "old-secret"
                && new_password.as_str() == "new-secret"
        ));
        let debug = format!("{request:?}");
        assert!(!debug.contains("old-secret"));
        assert!(!debug.contains("new-secret"));
        let encoded = serde_json::to_string(&request.effects).expect("security effect serializes");
        assert!(!encoded.contains("old-secret"));
        assert!(!encoded.contains("new-secret"));
        assert!(encoded.contains("REDACTED"));

        let duplicate = reduce(
            &request.state,
            UiAction::SubmitChangePassword {
                account: "account".into(),
                old_password: crate::effect::SecretText::new("old-secret"),
                new_password: crate::effect::SecretText::new("new-secret"),
                confirm_password: crate::effect::SecretText::new("new-secret"),
            },
        );
        assert!(duplicate.effects.is_empty());
        assert!(duplicate.state.security.change_password_pending);

        let rejected = reduce(
            &request.state,
            UiAction::ChangePasswordResult {
                success: false,
                message: "server rejected".into(),
            },
        );
        assert!(!rejected.state.security.change_password_pending);
        assert_eq!(
            rejected.state.security.panel,
            crate::state::UiSecurityPanel::ChangePassword
        );
        assert!(matches!(
            rejected.effects.as_slice(),
            [UiEffect::ShowNotice {
                message,
                is_error: true
            }] if message == "server rejected"
        ));

        let accepted = reduce(
            &request.state,
            UiAction::ChangePasswordResult {
                success: true,
                message: "changed".into(),
            },
        );
        assert_eq!(
            accepted.state.security.panel,
            crate::state::UiSecurityPanel::None
        );
        assert!(!accepted.state.security.change_password_pending);
    }

    #[test]
    fn safe_key_is_local_and_credits_remains_intentional_noop() {
        let login = UiState {
            screen: UiScreen::Login,
            ..Default::default()
        };
        let safe_key = reduce(&login, UiAction::SafeKey);
        assert_eq!(
            safe_key.state.security.panel,
            crate::state::UiSecurityPanel::SafeKey
        );
        assert!(safe_key.effects.is_empty());
        let closed = reduce(&safe_key.state, UiAction::CloseSafeKey);
        assert_eq!(
            closed.state.security.panel,
            crate::state::UiSecurityPanel::None
        );

        let credits = reduce(
            &UiState {
                screen: UiScreen::CharacterSelect,
                ..Default::default()
            },
            UiAction::OpenCredits,
        );
        assert_eq!(credits.effects, vec![UiEffect::Noop]);
    }

    #[test]
    fn legacy_apply_cancel_and_defaults_are_inert_from_crystal_options() {
        let opened = reduce(&game(), UiAction::OpenOptions).state;
        for action in [
            UiAction::SetWindowMode {
                mode: UiWindowMode::Fullscreen,
            },
            UiAction::ApplyOptions,
            UiAction::CancelOptions,
            UiAction::ResetOptionsToDefaults,
        ] {
            let transition = reduce(&opened, action);
            assert_eq!(transition.state, opened);
            assert!(transition.effects.is_empty());
        }
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
