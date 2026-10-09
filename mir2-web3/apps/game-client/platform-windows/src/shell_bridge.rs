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
    receiver: GatewayEventReceiver,
}

enum GatewayEventReceiver {
    Owned(Mutex<mpsc::Receiver<crate::gateway::NativeShellEnvelope>>),
    #[cfg(test)] Legacy(Mutex<mpsc::Receiver<NativeGatewayEvent>>),
}

impl GatewayEventInbox {
    pub(crate) fn new_owned(receiver:mpsc::Receiver<crate::gateway::NativeShellEnvelope>)->Self{Self {receiver:GatewayEventReceiver::Owned(Mutex::new(receiver))}}
    #[cfg(test)]
    pub fn new(receiver: mpsc::Receiver<NativeGatewayEvent>) -> Self {
        Self {
            receiver: GatewayEventReceiver::Legacy(Mutex::new(receiver)),
        }
    }

    fn drain(&self) -> Vec<crate::gateway::NativeShellEnvelope> {
        match &self.receiver {
            GatewayEventReceiver::Owned(receiver)=>receiver.lock().map(|r|r.try_iter().collect()).unwrap_or_default(),
            #[cfg(test)] GatewayEventReceiver::Legacy(receiver)=>receiver.lock().map(|r|r.try_iter().map(|event|crate::gateway::NativeShellEnvelope {stamp:None,event}).collect()).unwrap_or_default(),
        }
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
    for envelope in inbox.drain() {
        if let Some(stamp)=envelope.stamp {if !commands.apply_shell_stamp(stamp){continue;}}
        else if commands.ownership_fence().is_some(){continue;}
        let event=envelope.event;
        let previous_screen = shell.screen;
        // A password response is correlated to the one in-flight request in
        // NativeShellModel.  Never let a delayed response mutate a later
        // Login/Character/InGame state after the request was already closed.
        if matches!(
            &event,
            NativeGatewayEvent::ChangePasswordResult { .. }
                | NativeGatewayEvent::ChangePasswordBanned { .. }
        ) && !(shell.screen == NativeShellScreen::ChangePassword
            && shell.change_password_request_in_flight)
        {
            continue;
        }
        let connected = matches!(&event, NativeGatewayEvent::Connected);
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
        if shell.screen != previous_screen && std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
            eprintln!("[native-shell] transition {previous_screen:?} -> {:?}", shell.screen);
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
                send_shell_command(&commands,&mut shell,NativeOutboundCommand::Login {account_id,password});
                auto_login.submitted = true;
            }
        }
    }
    // Explicit development auto-login must wait for the original door animation,
    // including frames with no new Gateway event, before starting a character.
    if shell.screen == NativeShellScreen::CharacterSelect {
        if let Some(character_index) = auto_login.desired_character_index.take() {
            if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
                eprintln!("[native-shell] auto_start_attempt character_index={character_index}");
            }
            if shell.apply_ui_intent(NativeUiIntent::SelectCharacter { character_index })
                && shell.apply_ui_intent(NativeUiIntent::StartGame)
            {
                send_shell_command(&commands,&mut shell,NativeOutboundCommand::StartGame {character_index});
            }
        }
    }
}
/// A local before-commit failure completes only the original request class.
/// Callers qualify its immutable ticket before entering this existing reducer.
pub(crate) fn apply_local_control_not_sent(shell:&mut NativeShellModel,command:&NativeOutboundCommand){
    use NativeOutboundCommand as C;
    let relevant=match command {
        C::Login {..}=>shell.login_request_in_flight,
        C::StartGame {..}=>shell.start_game_request_in_flight,
        C::NewAccount {..}=>shell.register_request_in_flight,
        C::NewCharacter {..}=>shell.create_character_request_in_flight,
        C::DeleteCharacter {..}=>shell.delete_request_in_flight,
        C::ChangePassword {..}=>shell.change_password_request_in_flight,
        C::LogOut|C::Disconnect=>shell.logout_request_in_flight,
        _=>false,
    };
    if relevant{shell.apply_gateway_event(NativeGatewayEvent::OperationFailure {message:"local command definitely not sent".into()});}
}
fn send_shell_command(commands:&GatewayCommands,shell:&mut NativeShellModel,command:NativeOutboundCommand)->bool{
    if commands.send_command(GatewayCommand::Wire(command.clone())){true}else{apply_local_control_not_sent(shell,&command);false}
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
    let mut register_command_sent = false;
    let mut create_character_command_sent = false;
    let mut start_game_command_sent = false;
    let mut retry_command_sent = false;
    let mut logout_command_sent = false;
    for intent in pending {
        let command = match intent {
            NativeUiIntent::Login | NativeUiIntent::SafeKeyEnter
                if shell.login_request_in_flight && !login_command_sent =>
            {
                login_command_sent = true;
                commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::ClientVersion));
                Some(NativeOutboundCommand::Login {
                    account_id: shell.login.account.trim().to_owned(),
                    password: shell.login.password.clone(),
                })
            }
            NativeUiIntent::SubmitRegistration {
                account_id,
                password,
                birth_date_binary,
                user_name,
                secret_question,
                secret_answer,
                email_address,
                ..
            }
                if shell.register_request_in_flight && !register_command_sent =>
            {
                register_command_sent = true;
                commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::ClientVersion));
                Some(NativeOutboundCommand::NewAccount {
                    account_id,
                    password,
                    birth_date_binary,
                    user_name,
                    secret_question,
                    secret_answer,
                    email_address,
                })
            }
            NativeUiIntent::CreateCharacter {
                name,
                class_name,
                gender_name,
            } if shell.create_character_request_in_flight && !create_character_command_sent => {
                create_character_command_sent = true;
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
            NativeUiIntent::StartGame
                if shell.start_game_request_in_flight && !start_game_command_sent =>
            {
                start_game_command_sent = true;
                shell
                    .selected_character_index
                    .map(|character_index| NativeOutboundCommand::StartGame { character_index })
            }
            NativeUiIntent::Retry if shell.retry_request_in_flight && !retry_command_sent => {
                retry_command_sent = true;
                if !commands.send_command(GatewayCommand::Connect){shell.apply_gateway_event(NativeGatewayEvent::OperationFailure {message:"local reconnect request definitely not sent".into()});}
                None
            }
            NativeUiIntent::Logout if shell.logout_request_in_flight && !logout_command_sent => {
                logout_command_sent = true;
                Some(NativeOutboundCommand::LogOut)
            }
            NativeUiIntent::OpenCharacterCreate
            | NativeUiIntent::CancelCharacterCreate
            | NativeUiIntent::SelectCharacter { .. }
            | NativeUiIntent::Login
            | NativeUiIntent::OpenRegistration
            | NativeUiIntent::CancelRegistration
            | NativeUiIntent::SubmitRegistration { .. }
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
            send_shell_command(&commands,&mut shell,command);
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
    fn development_auto_start_waits_for_door_even_without_another_gateway_event() {
        let config = NativeAutoLogin {
            account_id: "door-test".into(),
            password: "test".into(),
            character_index: Some(7),
        };
        let mut shell = initial_shell_model(Some(&config));
        shell.screen = NativeShellScreen::Authenticating;
        shell.login_request_in_flight = true;
        let (events, inbox) = mpsc::channel();
        let (commands, sent) = mpsc::channel();
        let mut app = bevy::prelude::App::new();
        app.insert_resource(shell);
        app.insert_resource(GatewayEventInbox::new(inbox));
        app.insert_resource(GatewayCommands::new(commands));
        app.insert_resource(NativeAutoLoginFlow::from_config(Some(&config)));
        app.add_systems(bevy::prelude::Update, drain_gateway_events);
        events
            .send(NativeGatewayEvent::LoginSuccess {
                account: "door-test".into(),
                characters: vec![CharacterSummary::new(7, "Hero", 1, "Warrior", "Male")],
            })
            .unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            NativeShellScreen::OpeningLogin
        );
        assert!(sent.try_recv().is_err());
        app.world_mut()
            .resource_mut::<NativeShellModel>()
            .advance_login_opening(std::time::Duration::from_millis(1800));
        app.update();
        assert!(matches!(
            sent.try_recv(),
            Ok(GatewayCommand::Wire(NativeOutboundCommand::StartGame {
                character_index: 7
            }))
        ));
        app.update();
        assert!(sent.try_recv().is_err());
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
    fn registration_forwards_the_submitted_crystal_packet_fields() {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::Registration;
        shell.register_request_in_flight = true;
        let (commands, _) = forward(shell, [NativeUiIntent::SubmitRegistration {
            account_id: "player".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
            birth_date: "2000-01-01".to_owned(),
            birth_date_binary: 630_822_816_000_000_000,
            user_name: "Player Name".to_owned(),
            secret_question: "pet?".to_owned(),
            secret_answer: "cat".to_owned(),
            email_address: "player@example.test".to_owned(),
        }]);
        assert!(commands.iter().any(|command| matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::NewAccount {
                account_id,
                password,
                birth_date_binary: 630_822_816_000_000_000,
                user_name,
                secret_question,
                secret_answer,
                email_address,
            }) if account_id == "player"
                && password == "secret"
                && user_name == "Player Name"
                && secret_question == "pet?"
                && secret_answer == "cat"
                && email_address == "player@example.test"
        )));
        assert_eq!(
            commands
                .iter()
                .filter(|command| matches!(
                    command,
                    GatewayCommand::Wire(NativeOutboundCommand::ClientVersion)
                ))
                .count(),
            1
        );
    }

    #[test]
    fn duplicate_manual_intents_still_forward_once_per_logical_operation() {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::Authenticating;
        shell.login.account = "player".to_owned();
        shell.login.password = "secret".to_owned();
        shell.login_request_in_flight = true;
        let (commands, _) = forward(shell, [NativeUiIntent::Login, NativeUiIntent::Login]);
        assert_eq!(
            commands
                .iter()
                .filter(|command| matches!(
                    command,
                    GatewayCommand::Wire(NativeOutboundCommand::Login { .. })
                ))
                .count(),
            1
        );
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

    fn apply_gateway_event_to_shell(
        shell: NativeShellModel,
        event: NativeGatewayEvent,
    ) -> NativeShellModel {
        let (event_sender, event_receiver) = mpsc::channel();
        event_sender
            .send(event)
            .expect("gateway event should queue");
        let (command_sender, _command_receiver) = mpsc::channel();
        let mut app = bevy::prelude::App::new();
        app.insert_resource(shell);
        app.insert_resource(GatewayEventInbox::new(event_receiver));
        app.insert_resource(crate::input::GatewayCommands::new(command_sender));
        app.insert_resource(NativeAutoLoginFlow::default());
        app.add_systems(bevy::prelude::Update, drain_gateway_events);
        app.update();
        app.world().resource::<NativeShellModel>().clone()
    }

    fn pending_change_password_shell() -> NativeShellModel {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::ChangePassword;
        assert!(shell.apply_ui_intent(NativeUiIntent::SubmitChangePassword {
            account_id: "account".to_owned(),
            old_password: "oldpw".to_owned(),
            new_password: "newpw".to_owned(),
            confirm_password: "newpw".to_owned(),
        }));
        shell.mark_change_password_command_sent();
        shell
    }

    #[test]
    fn change_password_success_clears_pending_and_returns_to_login() {
        let shell = apply_gateway_event_to_shell(
            pending_change_password_shell(),
            NativeGatewayEvent::ChangePasswordResult { result: 6 },
        );
        assert_eq!(shell.screen, NativeShellScreen::Login);
        assert!(!shell.change_password_request_in_flight);
        assert!(!shell.change_password_command_sent);
        assert!(shell.change_password.old_password.is_empty());
        assert!(shell.change_password.new_password.is_empty());
        assert!(shell
            .notice
            .as_ref()
            .is_some_and(|notice| notice.message.contains("success")));
    }

    #[test]
    fn change_password_failure_clears_pending_and_keeps_form_without_secret_notice() {
        let shell = apply_gateway_event_to_shell(
            pending_change_password_shell(),
            NativeGatewayEvent::ChangePasswordResult { result: 2 },
        );
        assert_eq!(shell.screen, NativeShellScreen::ChangePassword);
        assert!(!shell.change_password_request_in_flight);
        assert!(!shell.change_password_command_sent);
        let notice = shell.notice.expect("failure notice");
        assert!(notice.message.contains("current password"));
        assert!(!notice.message.contains("oldpw"));
        assert!(!notice.message.contains("newpw"));
    }

    #[test]
    fn change_password_banned_clears_pending_and_returns_to_login_without_secret_notice() {
        let shell = apply_gateway_event_to_shell(
            pending_change_password_shell(),
            NativeGatewayEvent::ChangePasswordBanned {
                reason: "manual review".to_owned(),
                expiry: Some("2030-01-01".to_owned()),
            },
        );
        assert_eq!(shell.screen, NativeShellScreen::Login);
        assert!(!shell.change_password_request_in_flight);
        assert!(!shell.change_password_command_sent);
        let notice = shell.notice.expect("banned notice");
        assert!(notice.message.contains("manual review"));
        assert!(notice.message.contains("2030-01-01"));
        assert!(!notice.message.contains("oldpw"));
        assert!(!notice.message.contains("newpw"));
    }

    #[test]
    fn late_change_password_reply_without_pending_request_is_ignored_fail_closed() {
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::Login;
        shell.notice = Some(mir2_client_bevy::native_shell::ShellNotice::info(
            "still on login",
        ));
        let shell = apply_gateway_event_to_shell(
            shell,
            NativeGatewayEvent::ChangePasswordResult { result: 6 },
        );
        assert_eq!(shell.screen, NativeShellScreen::Login);
        assert!(!shell.change_password_request_in_flight);
        assert!(!shell.change_password_command_sent);
        assert_eq!(
            shell.notice.as_ref().map(|notice| notice.message.as_str()),
            Some("still on login")
        );
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

#[cfg(test)]
mod ownership_control_tests {
    use super::*;
    #[test]
    fn not_connected_bounded_login_completes_local_busy_without_server_rejection(){
        let (sender,_receiver)=crate::gateway::command_channel(8);let mut shell=NativeShellModel::default();shell.screen=NativeShellScreen::Authenticating;shell.login_request_in_flight=true;
        let mut intents=NativeUiIntentQueue::default();intents.push(NativeUiIntent::Login);
        let mut app=bevy::prelude::App::new();app.insert_resource(shell);app.insert_resource(intents);app.insert_resource(GatewayCommands::new(sender));app.add_systems(bevy::app::Update,forward_native_ui_intents);app.update();
        let shell=app.world().resource::<NativeShellModel>();assert_eq!(shell.screen,NativeShellScreen::Login);assert!(!shell.login_request_in_flight);
    }
    #[test]
    fn full_bounded_queue_start_game_completes_local_busy(){
        let (sender,_receiver)=crate::gateway::command_channel(8);let fence=sender.ownership_fence().unwrap();let stamp=fence.test_world_ready(7,0);
        for _ in 0..8{sender.send_with_stamp(GatewayCommand::Wire(NativeOutboundCommand::MagicKey {request_id:1,spell:"FireBall".into(),key:1,old_key:0}),Some(stamp)).unwrap();}
        let commands=GatewayCommands::new(sender);assert!(commands.apply_shell_stamp(stamp));
        let mut shell=NativeShellModel::default();shell.screen=NativeShellScreen::StartingGame;shell.start_game_request_in_flight=true;shell.selected_character_index=Some(3);
        let mut intents=NativeUiIntentQueue::default();intents.push(NativeUiIntent::StartGame);
        let mut app=bevy::prelude::App::new();app.insert_resource(commands);app.insert_resource(shell);app.insert_resource(intents);app.add_systems(bevy::app::Update,forward_native_ui_intents);app.update();
        let shell=app.world().resource::<NativeShellModel>();assert_eq!(shell.screen,NativeShellScreen::CharacterSelect);assert!(!shell.start_game_request_in_flight);assert!(!shell.start_game_acknowledged);
    }
}
