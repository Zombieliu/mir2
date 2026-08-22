//! Bevy main-thread bridge for the native shell and async Gateway owner.

use std::sync::{mpsc, Mutex};

use bevy::prelude::{Res, ResMut, Resource};
use mir2_client_bevy::native_shell::{
    NativeGatewayEvent, NativeShellModel, NativeShellScreen, NativeUiIntent, NativeUiIntentQueue,
};

use crate::{
    gateway::GatewayCommand, input::GatewayCommands, native_protocol::NativeOutboundCommand,
    session_config::NativeAutoLogin,
};

/// Thread-safe receiver wrapper accepted as a Bevy resource.
#[derive(Resource)]
pub struct GatewayEventInbox {
    receiver: Mutex<mpsc::Receiver<NativeGatewayEvent>>,
}

impl GatewayEventInbox {
    pub fn new(receiver: mpsc::Receiver<NativeGatewayEvent>) -> Self {
        Self {
            receiver: Mutex::new(receiver),
        }
    }

    fn drain(&self) -> Vec<NativeGatewayEvent> {
        let receiver = self
            .receiver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        receiver.try_iter().collect()
    }
}

/// Optional, explicit environment-driven development flow. Normal launches
/// leave this disabled and start at the visible Login screen.
#[derive(Debug, Default, Resource)]
pub struct NativeAutoLoginFlow {
    enabled: bool,
    submitted: bool,
    desired_character_index: Option<i32>,
}

impl NativeAutoLoginFlow {
    pub fn from_config(auto_login: Option<&NativeAutoLogin>) -> Self {
        match auto_login {
            Some(auto_login) => Self {
                enabled: true,
                submitted: false,
                desired_character_index: auto_login.character_index,
            },
            None => Self::default(),
        }
    }
}

pub fn initial_shell_model(auto_login: Option<&NativeAutoLogin>) -> NativeShellModel {
    let mut model = NativeShellModel::default();
    if let Some(auto_login) = auto_login {
        model.login.account = auto_login.account_id.clone();
        model.login.password = auto_login.password.clone();
    }
    model
}

/// Apply Gateway callbacks on the Bevy main thread. This is the only system
/// that mutates shell state from network input.
pub fn drain_gateway_events(
    mut shell: ResMut<NativeShellModel>,
    inbox: Res<GatewayEventInbox>,
    commands: Res<GatewayCommands>,
    mut auto_login: ResMut<NativeAutoLoginFlow>,
) {
    for event in inbox.drain() {
        let connected = matches!(&event, NativeGatewayEvent::Connected);
        let login_succeeded = matches!(&event, NativeGatewayEvent::LoginSuccess { .. });
        let reconnect_bootstrap = match &event {
            NativeGatewayEvent::PlayerBootstrapped { ref character }
                if matches!(
                    shell.screen,
                    NativeShellScreen::InGame | NativeShellScreen::ConnectionLost
                ) =>
            {
                Some(character.clone())
            }
            _ => None,
        };
        if connected && shell.screen == NativeShellScreen::ConnectionLost {
            // A resume deadline/rejection emits Disconnect first, then a
            // fresh opt-in connection emits Connected. Re-enter the visible
            // login state without letting the generic reducer ignore it.
            shell.screen = NativeShellScreen::Login;
            shell.characters.clear();
            shell.selected_character_index = None;
            shell.active_character = None;
            shell.notice = None;
            auto_login.submitted = false;
        } else if let Some(character) = reconnect_bootstrap {
            // The resumed world snapshot is authoritative and arrives after
            // sessionResumed. The normal StartingGame-only transition must not
            // reject this synthetic bootstrap while the shell stayed InGame.
            shell.screen = NativeShellScreen::InGame;
            shell.active_character = Some(character);
            shell.notice = None;
        } else {
            shell.apply_gateway_event(event);
        }

        if connected
            && auto_login.enabled
            && !auto_login.submitted
            && shell.screen == NativeShellScreen::Login
            && shell.login.is_ready()
        {
            let account_id = shell.login.account.trim().to_owned();
            let password = shell.login.password.clone();
            if shell.apply_ui_intent(NativeUiIntent::Login) {
                commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::ClientVersion));
                commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::Login {
                    account_id,
                    password,
                }));
                auto_login.submitted = true;
            }
        }

        if login_succeeded {
            if let Some(character_index) = auto_login.desired_character_index.take() {
                if shell.apply_ui_intent(NativeUiIntent::SelectCharacter { character_index })
                    && shell.apply_ui_intent(NativeUiIntent::StartGame)
                {
                    commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::StartGame {
                        character_index,
                    }));
                }
            }
        }
    }
}

/// Forward already-validated widget intents to the Gateway owner. Local-only
/// navigation/selection intents are intentionally ignored here.
pub fn forward_native_ui_intents(
    mut shell: ResMut<NativeShellModel>,
    mut intents: ResMut<NativeUiIntentQueue>,
    commands: Res<GatewayCommands>,
) {
    let pending = intents.drain().collect::<Vec<_>>();
    let mut login_command_sent = false;
    for intent in pending {
        let command = match intent {
            NativeUiIntent::Login | NativeUiIntent::SafeKeyEnter
                if shell.screen == NativeShellScreen::Authenticating && !login_command_sent =>
            {
                login_command_sent = true;
                commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::ClientVersion));
                Some(NativeOutboundCommand::Login {
                    account_id: shell.login.account.trim().to_owned(),
                    password: shell.login.password.clone(),
                })
            }
            NativeUiIntent::RegisterAccount if shell.screen == NativeShellScreen::Login => {
                let account_id = shell.login.account.trim().to_owned();
                commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::ClientVersion));
                Some(NativeOutboundCommand::NewAccount {
                    account_id: account_id.clone(),
                    password: shell.login.password.clone(),
                    birth_date_binary: 0,
                    user_name: account_id,
                    secret_question: String::new(),
                    secret_answer: String::new(),
                    email_address: String::new(),
                })
            }
            NativeUiIntent::CreateCharacter {
                name,
                class_name,
                gender_name,
            } if shell.screen == NativeShellScreen::CharacterCreate => {
                Some(NativeOutboundCommand::NewCharacter {
                    name: name.trim().to_owned(),
                    gender: gender_name,
                    class: class_name,
                })
            }
            NativeUiIntent::SubmitChangePassword { .. }
                if shell.screen == NativeShellScreen::ChangePassword
                    && shell.change_password_command_pending() =>
            {
                shell.mark_change_password_command_sent();
                Some(NativeOutboundCommand::ChangePassword {
                    account_id: shell.change_password.account_id.clone(),
                    current_password: shell.change_password.old_password.clone(),
                    new_password: shell.change_password.new_password.clone(),
                })
            }
            NativeUiIntent::ConfirmDeleteCharacter
                if matches!(shell.screen, NativeShellScreen::DeleteConfirm { .. }) =>
            {
                let Some(idx) = shell.delete_command_pending() else {
                    continue;
                };
                shell.mark_delete_command_sent();
                Some(NativeOutboundCommand::DeleteCharacter {
                    character_index: idx,
                })
            }
            NativeUiIntent::StartGame if shell.screen == NativeShellScreen::StartingGame => shell
                .selected_character_index
                .map(|character_index| NativeOutboundCommand::StartGame { character_index }),
            NativeUiIntent::Retry if shell.screen == NativeShellScreen::Connecting => {
                commands.send_command(GatewayCommand::Connect);
                None
            }
            NativeUiIntent::Logout if shell.screen == NativeShellScreen::Login => {
                Some(NativeOutboundCommand::LogOut)
            }
            NativeUiIntent::OpenCharacterCreate
            | NativeUiIntent::CancelCharacterCreate
            | NativeUiIntent::SelectCharacter { .. }
            | NativeUiIntent::Login
            | NativeUiIntent::RegisterAccount
            | NativeUiIntent::CreateCharacter { .. }
            | NativeUiIntent::DeleteCharacter { .. }
            | NativeUiIntent::StartGame
            | NativeUiIntent::Retry
            | NativeUiIntent::Logout
            | NativeUiIntent::OpenChangePassword
            | NativeUiIntent::SubmitChangePassword { .. }
            | NativeUiIntent::CancelChangePassword
            | NativeUiIntent::OpenSafeKey
            | NativeUiIntent::CloseSafeKey
            | NativeUiIntent::SafeKeyFocusAccount
            | NativeUiIntent::SafeKeyFocusPassword
            | NativeUiIntent::SafeKeyPress { .. }
            | NativeUiIntent::SafeKeyDelete
            | NativeUiIntent::SafeKeyRandom
            | NativeUiIntent::SafeKeyEnter
            | NativeUiIntent::ConfirmDeleteCharacter
            | NativeUiIntent::CancelDeleteCharacter => None,
        };

        if let Some(command) = command {
            commands.send_command(GatewayCommand::Wire(command));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    use mir2_client_bevy::native_shell::CharacterSummary;

    fn forward(
        shell: NativeShellModel,
        queued: impl IntoIterator<Item = NativeUiIntent>,
    ) -> (Vec<GatewayCommand>, NativeShellModel) {
        let (sender, receiver) = mpsc::channel();
        let mut app = bevy::prelude::App::new();
        app.insert_resource(shell);
        let mut intents = NativeUiIntentQueue::default();
        for intent in queued {
            intents.push(intent);
        }
        app.insert_resource(intents);
        app.insert_resource(crate::input::GatewayCommands::new(sender));
        app.add_systems(bevy::prelude::Update, forward_native_ui_intents);
        app.update();
        let shell = app.world().resource::<NativeShellModel>().clone();
        (receiver.try_iter().collect(), shell)
    }

    #[test]
    fn normal_launch_has_no_credentials_or_auto_submit() {
        let model = initial_shell_model(None);
        let flow = NativeAutoLoginFlow::from_config(None);
        assert!(model.login.account.is_empty());
        assert!(model.login.password.is_empty());
        assert!(!flow.enabled);
        assert!(!flow.submitted);
    }

    #[test]
    fn explicit_auto_login_prefills_but_debug_stays_redacted_upstream() {
        let config = NativeAutoLogin {
            account_id: "player".to_owned(),
            password: "secret".to_owned(),
            character_index: Some(2),
        };
        let model = initial_shell_model(Some(&config));
        let flow = NativeAutoLoginFlow::from_config(Some(&config));
        assert_eq!(model.login.account, "player");
        assert_eq!(model.login.password, "secret");
        assert!(flow.enabled);
        assert_eq!(flow.desired_character_index, Some(2));
    }

    #[test]
    fn delete_confirmation_forwards_exactly_once_for_selected_index() {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::CharacterSelect;
        shell.characters = vec![CharacterSummary::new(7, "Hero", 1, "Warrior", "Male")];
        shell.selected_character_index = Some(7);
        assert!(shell.apply_ui_intent(NativeUiIntent::DeleteCharacter { character_index: 7 }));
        // The UI state machine validates and marks the confirmation pending
        // before the queued intent reaches this transport-only bridge.
        assert!(shell.apply_ui_intent(NativeUiIntent::ConfirmDeleteCharacter));

        let (commands, shell) = forward(
            shell,
            [
                NativeUiIntent::ConfirmDeleteCharacter,
                NativeUiIntent::ConfirmDeleteCharacter,
            ],
        );
        let delete_indices = commands
            .iter()
            .filter_map(|command| match command {
                GatewayCommand::Wire(NativeOutboundCommand::DeleteCharacter {
                    character_index,
                }) => Some(*character_index),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(delete_indices, vec![7]);

        let (commands, _) = forward(shell, [NativeUiIntent::ConfirmDeleteCharacter]);
        assert!(commands.is_empty());
    }

    #[test]
    fn change_password_forwards_account_id_and_gateway_fields_once() {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::ChangePassword;
        let intent = NativeUiIntent::SubmitChangePassword {
            account_id: "account".to_owned(),
            old_password: "oldpw".to_owned(),
            new_password: "newpw".to_owned(),
            confirm_password: "newpw".to_owned(),
        };
        assert!(shell.apply_ui_intent(intent.clone()));

        let (commands, _) = forward(shell, [intent.clone(), intent]);
        let change_password = commands
            .iter()
            .filter_map(|command| match command {
                GatewayCommand::Wire(NativeOutboundCommand::ChangePassword {
                    account_id,
                    current_password,
                    new_password,
                }) => Some((account_id, current_password, new_password)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(change_password.len(), 1);
        assert_eq!(change_password[0].0.as_str(), "account");
        assert_eq!(change_password[0].1.as_str(), "oldpw");
        assert_eq!(change_password[0].2.as_str(), "newpw");
    }

    #[test]
    fn resumed_bootstrap_refreshes_ingame_shell_without_login_or_start_game() {
        let (event_sender, event_receiver) = mpsc::channel();
        let (command_sender, command_receiver) = mpsc::channel();
        let resumed_character = CharacterSummary::new(4, "ResumedHero", 18, "Wizard", "Male");

        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        shell.active_character = Some(CharacterSummary::new(4, "StaleHero", 17, "Wizard", "Male"));
        shell.notice = Some(mir2_client_bevy::native_shell::ShellNotice::warn(
            "resume in progress",
        ));

        event_sender
            .send(NativeGatewayEvent::PlayerBootstrapped {
                character: resumed_character.clone(),
            })
            .expect("resume bootstrap event should be queued");

        let mut app = bevy::prelude::App::new();
        app.insert_resource(shell);
        app.insert_resource(GatewayEventInbox::new(event_receiver));
        app.insert_resource(crate::input::GatewayCommands::new(command_sender));
        app.insert_resource(NativeAutoLoginFlow::default());
        app.add_systems(bevy::prelude::Update, drain_gateway_events);
        app.update();

        let shell = app.world().resource::<NativeShellModel>();
        assert_eq!(shell.screen, NativeShellScreen::InGame);
        assert_eq!(shell.active_character.as_ref(), Some(&resumed_character));
        assert_eq!(shell.notice, None);
        assert!(command_receiver.try_iter().all(|command| {
            !matches!(
                command,
                GatewayCommand::Wire(NativeOutboundCommand::Login { .. })
                    | GatewayCommand::Wire(NativeOutboundCommand::StartGame { .. })
            )
        }));
    }
}
