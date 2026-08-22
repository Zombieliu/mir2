//! Android native shell foundation for the shared Mir2 Bevy runtime.
//!
//! The library is deliberately a `cdylib`. On Android, Bevy's supported
//! `#[bevy_main]` macro emits `android_main`, stores the Activity handle for
//! Winit/asset loading, then enters this same app construction path.

pub mod android_input;
pub mod gateway_bridge;

use android_input::{
    apply_android_lifecycle_messages, collect_android_back_key, route_android_input_messages,
    AndroidInputMessage, AndroidLifecycleEffects, AndroidLifecycleMessage, AndroidMotionQueue,
    AndroidShellState, AndroidUiActionQueue,
};
use bevy::app::AppExit;
use bevy::prelude::*;
use gateway_bridge::{
    clear_bounded_inbound_transaction, drain_bounded_inbound_into_models,
    enqueue_game_shop_purchase, enqueue_security_request, AndroidGatewayInboundQueue,
    AndroidGatewayOutboundQueue,
};
use mir2_bevy_runtime::{build_runtime_app, RuntimeWindowSpec};
use mir2_ui_core::{effect::UiEffect, reducer::reduce, state::UiState};

#[derive(Debug, Default, Resource)]
pub struct AndroidUiEffects(pub Vec<UiEffect>);

/// Installs platform-only routing without changing shared UI or runtime crates.
pub struct AndroidShellPlugin;

impl Plugin for AndroidShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .init_resource::<AndroidShellState>()
            .init_resource::<AndroidUiActionQueue>()
            .init_resource::<AndroidMotionQueue>()
            .init_resource::<AndroidLifecycleEffects>()
            .init_resource::<AndroidUiEffects>()
            .init_resource::<AndroidGatewayOutboundQueue>()
            .init_resource::<AndroidGatewayInboundQueue>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<AppExit>()
            .add_message::<AndroidInputMessage>()
            .add_message::<AndroidLifecycleMessage>()
            .add_systems(
                Update,
                (
                    collect_android_back_key,
                    apply_android_lifecycle_messages,
                    drain_android_gateway_inbound,
                    route_android_input_messages,
                    apply_queued_ui_actions,
                )
                    .chain(),
            );
    }
}

fn drain_android_gateway_inbound(
    mut ui_state: ResMut<UiState>,
    mut outbound: ResMut<AndroidGatewayOutboundQueue>,
    mut inbound: ResMut<AndroidGatewayInboundQueue>,
) {
    drain_bounded_inbound_into_models(&mut inbound, &mut ui_state, &mut outbound);
}

fn apply_queued_ui_actions(
    mut ui_state: ResMut<UiState>,
    mut queue: ResMut<AndroidUiActionQueue>,
    mut effects: ResMut<AndroidUiEffects>,
    mut gateway: ResMut<AndroidGatewayOutboundQueue>,
    mut inbound: ResMut<AndroidGatewayInboundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in std::mem::take(&mut queue.0) {
        let exit_requested = matches!(action, mir2_ui_core::action::UiAction::ExitApplication);
        let terminal_session = matches!(
            action,
            mir2_ui_core::action::UiAction::Logout
                | mir2_ui_core::action::UiAction::ExitApplication
        );
        if terminal_session {
            // Clear a possibly lost native purchase before enqueueing the
            // explicit logout command. The command itself is still retained.
            gateway.mark_terminal_reset();
            clear_bounded_inbound_transaction(&mut inbound);
        }
        if let mir2_ui_core::action::UiAction::GameShopBuy {
            g_index,
            quantity,
            price_type,
        } = action
        {
            let _ = enqueue_game_shop_purchase(
                &mut ui_state,
                &mut gateway,
                &mut inbound,
                g_index,
                quantity,
                price_type,
            );
            continue;
        }
        let transition = reduce(&ui_state, action);
        *ui_state = transition.state;
        for effect in transition.effects {
            match effect {
                UiEffect::GatewayCommand(command) => {
                    // Overflow is observable through queue status/counters;
                    // existing FIFO entries remain untouched.
                    let _ = gateway.enqueue(command);
                }
                UiEffect::SecurityRequest(request) => {
                    if enqueue_security_request(&mut gateway, &mut inbound, request).is_err() {
                        // The reducer already marked this request pending. If
                        // the bounded transport rejects it, clear that state
                        // through the same authoritative-result action so the
                        // form cannot become permanently stuck. Never include
                        // credentials or adapter internals in the notice.
                        let failed = reduce(
                            &ui_state,
                            mir2_ui_core::action::UiAction::ChangePasswordResult {
                                success: false,
                                message: "change-password request could not be queued".to_owned(),
                            },
                        );
                        *ui_state = failed.state;
                        effects.0.extend(failed.effects);
                    }
                }
                UiEffect::RequestObserve { allow: _ } => {
                    let authoritative_before_request = ui_state.observe_allowed;
                    if gateway
                        .enqueue(mir2_ui_core::effect::GatewayCommand::SendChat {
                            message: "@ALLOWOBSERVE".to_owned(),
                        })
                        .is_err()
                    {
                        // A rejected transport enqueue must not leave the
                        // request-only toggle permanently pending. Restore the
                        // last authoritative value without pretending the
                        // requested value was accepted by the server.
                        let failed = reduce(
                            &ui_state,
                            mir2_ui_core::action::UiAction::ObserveAuthoritativeChanged {
                                allow: authoritative_before_request,
                            },
                        );
                        *ui_state = failed.state;
                        effects.0.push(UiEffect::ShowNotice {
                            message: "observe request could not be queued".to_owned(),
                            is_error: true,
                        });
                    }
                }
                other => effects.0.push(other),
            }
        }
        if exit_requested {
            exit.write(AppExit::Success);
        }
    }
}

pub fn build_android_runtime_app() -> App {
    let mut app = build_runtime_app(RuntimeWindowSpec {
        width: 1280,
        height: 720,
        ..RuntimeWindowSpec::native("mir2-web3 (android)")
    });
    app.add_plugins(AndroidShellPlugin);
    app
}

/// The function name is intentional: Bevy's Android macro emits the
/// `android_main(AndroidApp)` symbol for the cdylib and initializes Winit with
/// the native Activity before calling this function.
#[cfg_attr(target_os = "android", bevy::prelude::bevy_main)]
pub fn main() {
    let mut app = build_android_runtime_app();
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::android_input::{AndroidInputEvent, AndroidLifecycleEvent, AndroidUiTarget};
    use mir2_ui_core::{
        action::UiAction,
        effect::UiEffect,
        state::{UiChatChannel, UiPanel, UiScreen, UiWindowMode},
    };

    fn in_game_app() -> App {
        let mut app = App::new();
        app.add_plugins(AndroidShellPlugin);
        app.world_mut().resource_mut::<UiState>().screen = UiScreen::InGame;
        app
    }

    fn send(app: &mut App, event: AndroidInputEvent) {
        app.world_mut().write_message(AndroidInputMessage(event));
        app.update();
    }

    fn take_effects(app: &mut App) -> Vec<UiEffect> {
        std::mem::take(&mut app.world_mut().resource_mut::<AndroidUiEffects>().0)
    }

    fn failure_receipt(request_id: &str, quantity: u8) -> String {
        format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{request_id}","success":false,"gIndex":31,"quantity":{quantity},"priceType":1,"code":"insufficientCurrency"}}"#
        )
    }

    #[test]
    fn gateway_effects_are_consumed_into_the_host_queue() {
        let mut app = App::new();
        app.add_plugins(AndroidShellPlugin);
        {
            let mut state = app.world_mut().resource_mut::<UiState>();
            state.screen = UiScreen::Login;
            state.login_account = "account".into();
            state.login_password = "password".into();
        }
        send(&mut app, AndroidInputEvent::Semantic(UiAction::Login));

        assert!(take_effects(&mut app).is_empty());
        let queue = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.status().overflow_count, 0);
    }

    #[test]
    fn change_password_effect_is_consumed_into_the_real_gateway_queue() {
        use mir2_ui_core::effect::SecretText;

        let mut app = App::new();
        app.add_plugins(AndroidShellPlugin);
        app.world_mut().resource_mut::<UiState>().screen = UiScreen::Login;
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::ChangePassword),
        );
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SubmitChangePassword {
                account: "account".to_owned(),
                old_password: SecretText::new("old-secret"),
                new_password: SecretText::new("new-secret"),
                confirm_password: SecretText::new("new-secret"),
            }),
        );

        assert!(take_effects(&mut app).is_empty());
        assert!(
            app.world()
                .resource::<UiState>()
                .security
                .change_password_pending
        );
        let queue = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert_eq!(queue.len(), 1);
        assert!(queue.change_password_in_flight());
    }

    #[test]
    fn rejected_change_password_enqueue_clears_pending_without_leaking_secrets() {
        use mir2_ui_core::effect::SecretText;

        let mut app = App::new();
        app.add_plugins(AndroidShellPlugin);
        app.world_mut().resource_mut::<UiState>().screen = UiScreen::Login;
        let mut full = gateway_bridge::AndroidGatewayOutboundQueue::with_capacity(1);
        full.enqueue(mir2_ui_core::effect::GatewayCommand::TownRevive)
            .unwrap();
        app.insert_resource(full);
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::ChangePassword),
        );
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SubmitChangePassword {
                account: "account".to_owned(),
                old_password: SecretText::new("old-secret"),
                new_password: SecretText::new("new-secret"),
                confirm_password: SecretText::new("new-secret"),
            }),
        );

        assert!(
            !app.world()
                .resource::<UiState>()
                .security
                .change_password_pending
        );
        let effects = take_effects(&mut app);
        assert_eq!(effects.len(), 1);
        let debug = format!("{effects:?}");
        assert!(debug.contains("could not be queued"));
        assert!(!debug.contains("old-secret"));
        assert!(!debug.contains("new-secret"));
    }

    #[test]
    fn observe_request_is_consumed_as_the_crystal_chat_command() {
        let mut app = in_game_app();
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::Options;

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::RequestObserve { allow: true }),
        );

        let state = app.world().resource::<UiState>();
        assert!(
            !state.observe_allowed,
            "the client must not accept its own request"
        );
        assert_eq!(state.observe_request_pending, Some(true));
        assert!(take_effects(&mut app).is_empty());

        let mut shell = crate::android_input::AndroidShellState::default();
        shell.lifecycle = crate::android_input::AndroidLifecycle::Foreground;
        shell.network = crate::android_input::AndroidNetwork::Available;
        let outbound = app
            .world_mut()
            .resource_mut::<gateway_bridge::AndroidGatewayOutboundQueue>()
            .drain_ready(&shell, 1);
        assert_eq!(outbound.len(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&outbound[0].json).unwrap(),
            serde_json::json!({"type":"chat","message":"@ALLOWOBSERVE"})
        );
    }

    #[test]
    fn rejected_observe_enqueue_restores_last_authoritative_value() {
        let mut app = in_game_app();
        let mut full = gateway_bridge::AndroidGatewayOutboundQueue::with_capacity(1);
        full.enqueue(mir2_ui_core::effect::GatewayCommand::TownRevive)
            .unwrap();
        app.insert_resource(full);
        {
            let mut state = app.world_mut().resource_mut::<UiState>();
            state.panel = UiPanel::Options;
            state.observe_allowed = false;
        }

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::RequestObserve { allow: true }),
        );

        let state = app.world().resource::<UiState>();
        assert!(!state.observe_allowed);
        assert_eq!(state.observe_request_pending, None);
        let effects = take_effects(&mut app);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            &effects[0],
            UiEffect::ShowNotice {
                message,
                is_error: true
            } if message == "observe request could not be queued"
        ));
    }

    #[test]
    fn android_production_adapter_rolls_back_ui_when_outbound_queue_is_full() {
        let mut app = in_game_app();
        let mut full = gateway_bridge::AndroidGatewayOutboundQueue::with_capacity(1);
        full.enqueue(mir2_ui_core::effect::GatewayCommand::TownRevive)
            .unwrap();
        app.insert_resource(full);
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::GameShop;

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::GameShopBuy {
                g_index: 31,
                quantity: 2,
                price_type: 1,
            }),
        );

        assert!(app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .is_none());
        let queue = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert!(queue.game_shop_pending().is_none());
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.status().overflow_count, 1);
    }

    #[test]
    fn android_production_inbound_receipt_releases_both_correlation_owners() {
        let mut app = in_game_app();
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::GameShop;
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::GameShopBuy {
                g_index: 31,
                quantity: 2,
                price_type: 1,
            }),
        );
        let request_id = app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .as_ref()
            .unwrap()
            .request_id
            .clone();

        let wrong = r#"{"protocol":"nativeGameShopReceiptV1","requestId":"gs-wrong","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"}"#;
        gateway_bridge::enqueue_native_game_shop_receipt(
            &mut app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>(),
            wrong,
        )
        .unwrap();
        app.update();
        assert!(app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .is_some());
        assert!(app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>()
            .game_shop_pending()
            .is_some());

        let exact = format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{}","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"}}"#,
            request_id
        );
        gateway_bridge::enqueue_native_game_shop_receipt(
            &mut app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>(),
            exact.clone(),
        )
        .unwrap();
        app.update();
        assert!(app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .is_none());
        assert!(app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>()
            .game_shop_pending()
            .is_none());
        assert_eq!(
            app.world()
                .resource::<gateway_bridge::AndroidGatewayOutboundQueue>()
                .len(),
            0,
            "accepted receipt removes any retained purchase frame"
        );
        gateway_bridge::enqueue_native_game_shop_receipt(
            &mut app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>(),
            exact,
        )
        .unwrap();
        app.update();
        assert_eq!(
            app.world()
                .resource::<gateway_bridge::AndroidGatewayInboundQueue>()
                .status()
                .unmatched_count,
            2,
            "wrong plus late duplicate are both ignored by the production Update path"
        );
    }

    #[test]
    fn android_inbound_overflow_marks_both_owners_unknown_without_replay() {
        let mut app = in_game_app();
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::GameShop;
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            }),
        );
        let request_id = app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .as_ref()
            .unwrap()
            .request_id
            .clone();
        {
            let mut inbound = app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>();
            for _ in 0..gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY {
                gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, "not-json").unwrap();
            }
            assert!(gateway_bridge::enqueue_native_game_shop_receipt(
                &mut inbound,
                format!(
                    r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{request_id}","success":false,"gIndex":31,"quantity":1,"priceType":1,"code":"insufficientCurrency"}}"#
                ),
            )
            .is_err());
        }
        app.update();

        let state = app.world().resource::<UiState>();
        assert!(state.game_shop_pending.is_none());
        assert!(state.game_shop_unknown);
        let outbound = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert!(outbound.game_shop_pending().is_none());
        assert!(outbound.game_shop_unknown());
        assert_eq!(outbound.len(), 0, "overflow cannot leave a replayable buy");
        let status = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayInboundQueue>()
            .status();
        assert_eq!(status.overflow_count, 1);
        assert_eq!(
            status.malformed_count as usize,
            gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY
        );
    }

    #[test]
    fn wrong_valid_receipt_then_overflow_marks_exact_pending_unknown() {
        let mut app = in_game_app();
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::GameShop;
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            }),
        );
        {
            let mut inbound = app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>();
            gateway_bridge::enqueue_native_game_shop_receipt(
                &mut inbound,
                failure_receipt("gs-wrong", 1),
            )
            .unwrap();
            for _ in 1..gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY {
                gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, "not-json").unwrap();
            }
            assert!(
                gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, "flood").is_err()
            );
        }

        app.update();

        let state = app.world().resource::<UiState>();
        assert!(state.game_shop_pending.is_none());
        assert!(state.game_shop_unknown);
        let outbound = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert!(outbound.game_shop_pending().is_none());
        assert!(outbound.game_shop_unknown());
        assert_eq!(outbound.len(), 0);
        let inbound = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayInboundQueue>();
        assert_eq!(inbound.status().overflow_count, 1);
        assert_eq!(inbound.status().unmatched_count, 1);
        assert_eq!(
            inbound.status().malformed_count as usize,
            gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY - 1
        );
    }

    #[test]
    fn exact_receipt_survives_wrong_invalid_duplicate_and_flood_once() {
        let mut app = in_game_app();
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::GameShop;
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            }),
        );
        let request_id = app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .as_ref()
            .unwrap()
            .request_id
            .clone();
        let exact = failure_receipt(&request_id, 1);
        let invalid = format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{request_id}","success":true,"gIndex":31,"quantity":1,"priceType":1,"code":"commitFailed","mailId":99}}"#
        );
        {
            let mut inbound = app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>();
            gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, exact.clone()).unwrap();
            gateway_bridge::enqueue_native_game_shop_receipt(
                &mut inbound,
                failure_receipt("gs-wrong", 1),
            )
            .unwrap();
            gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, invalid).unwrap();
            gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, exact.clone()).unwrap();
            for _ in 4..gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY {
                gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, "not-json").unwrap();
            }
            assert!(
                gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, "flood").is_err()
            );
        }

        app.update();

        let state = app.world().resource::<UiState>();
        assert!(state.game_shop_pending.is_none());
        assert!(!state.game_shop_unknown);
        assert_eq!(
            state
                .game_shop_last_receipt
                .as_ref()
                .map(|receipt| receipt.request_id.as_str()),
            Some(request_id.as_str())
        );
        let outbound = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert!(outbound.game_shop_pending().is_none());
        assert!(!outbound.game_shop_unknown());
        assert_eq!(outbound.len(), 0);
        let inbound = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayInboundQueue>();
        assert_eq!(inbound.status().overflow_count, 1);
        assert_eq!(inbound.status().unmatched_count, 2);
        assert_eq!(
            inbound.status().malformed_count as usize,
            gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY - 3
        );
    }

    #[test]
    fn receipt_without_pending_is_quarantined_even_when_queue_overflows() {
        let mut app = in_game_app();
        {
            let mut inbound = app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>();
            gateway_bridge::enqueue_native_game_shop_receipt(
                &mut inbound,
                failure_receipt("gs-orphan", 1),
            )
            .unwrap();
            for _ in 1..gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY {
                gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, "not-json").unwrap();
            }
            assert!(
                gateway_bridge::enqueue_native_game_shop_receipt(&mut inbound, "flood").is_err()
            );
        }

        app.update();

        let state = app.world().resource::<UiState>();
        assert!(state.game_shop_pending.is_none());
        assert!(!state.game_shop_unknown);
        let outbound = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert!(outbound.game_shop_pending().is_none());
        assert!(!outbound.game_shop_unknown());
        let inbound = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayInboundQueue>();
        assert_eq!(inbound.status().overflow_count, 1);
        assert_eq!(inbound.status().unmatched_count, 1);
        assert_eq!(
            inbound.status().malformed_count as usize,
            gateway_bridge::ANDROID_GATEWAY_INBOUND_CAPACITY - 1
        );
    }

    #[test]
    fn android_malformed_receipt_through_update_does_not_release_pending() {
        let mut app = in_game_app();
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::GameShop;
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            }),
        );
        gateway_bridge::enqueue_native_game_shop_receipt(
            &mut app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>(),
            "not-json",
        )
        .unwrap();
        app.update();

        assert!(app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .is_some());
        assert!(app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>()
            .game_shop_pending()
            .is_some());
        assert_eq!(
            app.world()
                .resource::<gateway_bridge::AndroidGatewayInboundQueue>()
                .status()
                .malformed_count,
            1
        );
    }

    #[test]
    fn android_destroy_clears_pending_purchase_without_replay() {
        let mut app = in_game_app();
        app.world_mut().resource_mut::<UiState>().panel = UiPanel::GameShop;
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::GameShopBuy {
                g_index: 31,
                quantity: 1,
                price_type: 1,
            }),
        );
        assert!(app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .is_some());
        assert!(app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>()
            .game_shop_pending()
            .is_some());

        let request_id = app
            .world()
            .resource::<UiState>()
            .game_shop_pending
            .as_ref()
            .unwrap()
            .request_id
            .clone();
        gateway_bridge::enqueue_native_game_shop_receipt(
            &mut app
                .world_mut()
                .resource_mut::<gateway_bridge::AndroidGatewayInboundQueue>(),
            format!(
                r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{request_id}","success":false,"gIndex":31,"quantity":1,"priceType":1,"code":"insufficientCurrency"}}"#
            ),
        )
        .unwrap();

        app.world_mut()
            .write_message(AndroidLifecycleMessage(AndroidLifecycleEvent::Destroy));
        app.update();

        let state = app.world().resource::<UiState>();
        assert!(state.game_shop_pending.is_none());
        assert!(state.game_shop_unknown);
        let queue = app
            .world()
            .resource::<gateway_bridge::AndroidGatewayOutboundQueue>();
        assert!(queue.game_shop_pending().is_none());
        assert!(queue.game_shop_unknown());
        assert_eq!(queue.len(), 0);
        assert_eq!(
            app.world()
                .resource::<gateway_bridge::AndroidGatewayInboundQueue>()
                .status()
                .unmatched_count,
            1,
            "Destroy runs before inbound drain, so the late receipt cannot confirm"
        );
    }

    #[test]
    fn plugin_reduces_a_semantic_android_tap_through_ui_core() {
        let mut app = in_game_app();
        send(
            &mut app,
            AndroidInputEvent::Tap {
                target: AndroidUiTarget::Mail,
            },
        );
        assert!(app.world().resource::<UiState>().is_mail_open());
    }

    #[test]
    fn android_crystal_options_commit_immediately_and_platform_window_mode_is_separate() {
        let mut app = in_game_app();
        {
            let mut state = app.world_mut().resource_mut::<UiState>();
            state.options.music_volume = 55;
            state.options.window_mode = UiWindowMode::Windowed;
        }

        send(
            &mut app,
            AndroidInputEvent::Tap {
                target: AndroidUiTarget::Options,
            },
        );
        assert_eq!(app.world().resource::<UiState>().panel, UiPanel::Options);
        assert!(app.world().resource::<UiState>().options_draft.is_none());

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SetMusicVolume { volume: 12 }),
        );
        let committed = app.world().resource::<UiState>();
        assert_eq!(committed.options.music_volume, 12);
        assert_eq!(committed.options.window_mode, UiWindowMode::Windowed);
        assert!(committed.options_draft.is_none());
        let effects = take_effects(&mut app);
        assert_eq!(effects.len(), 2);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            UiEffect::ApplyAudioSettings {
                music_enabled: true,
                music_volume: 12,
                sound_enabled: true,
                sound_volume: 80,
            }
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            UiEffect::PersistOptions { options }
                if options.music_volume == 12
                    && options.window_mode == UiWindowMode::Windowed
        )));

        // Legacy window controls are deliberately inert inside Crystal's
        // Options panel; the original panel closes without apply/cancel.
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SetWindowMode {
                mode: UiWindowMode::Fullscreen,
            }),
        );
        assert!(take_effects(&mut app).is_empty());

        send(&mut app, AndroidInputEvent::Back);
        let closed = app.world().resource::<UiState>();
        assert_eq!(closed.panel, UiPanel::None);
        assert_eq!(closed.options.music_volume, 12);
        assert_eq!(closed.options.window_mode, UiWindowMode::Windowed);
        assert!(take_effects(&mut app).is_empty());

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::OpenPlatformSettings),
        );
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SetPlatformWindowMode {
                mode: UiWindowMode::Fullscreen,
            }),
        );
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::ApplyPlatformSettings),
        );

        let applied = app.world().resource::<UiState>();
        assert_eq!(applied.panel, UiPanel::None);
        assert_eq!(applied.options.music_volume, 12);
        assert_eq!(applied.options.window_mode, UiWindowMode::Fullscreen);
        assert!(applied.platform_settings_draft.is_none());
        let effects = take_effects(&mut app);
        assert_eq!(effects.len(), 2);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            UiEffect::ApplyWindowMode {
                mode: UiWindowMode::Fullscreen
            }
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            UiEffect::PersistOptions { options }
                if options.window_mode == UiWindowMode::Fullscreen
        )));
        assert!(take_effects(&mut app).is_empty());
    }

    #[test]
    fn android_chat_settings_semantics_keep_trade_outside_crystal_eight_checkboxes() {
        let mut app = in_game_app();
        app.world_mut()
            .resource_mut::<UiState>()
            .chat_settings
            .filter_trade = true;

        send(
            &mut app,
            AndroidInputEvent::Tap {
                target: AndroidUiTarget::ChatSettings,
            },
        );
        assert_eq!(
            app.world().resource::<UiState>().panel,
            UiPanel::ChatSettings
        );

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SetAllChatFilterVisibility { visible: false }),
        );
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SetChatTransparency { transparent: true }),
        );
        let staged = app.world().resource::<UiState>();
        assert!(!staged.chat_settings.filter_normal);
        assert!(staged.chat_settings_draft.as_ref().unwrap().filter_normal);
        assert!(staged.chat_settings_draft.as_ref().unwrap().filter_guild);
        assert!(staged.chat_settings_draft.as_ref().unwrap().filter_trade);
        assert!(staged.chat_settings_draft.as_ref().unwrap().transparent);

        send(&mut app, AndroidInputEvent::Back);
        let cancelled = app.world().resource::<UiState>();
        assert_eq!(cancelled.panel, UiPanel::None);
        assert!(!cancelled.chat_settings.filter_normal);
        assert!(!cancelled.chat_settings.filter_guild);
        assert!(cancelled.chat_settings.filter_trade);
        assert!(!cancelled.chat_settings.transparent);
        assert!(take_effects(&mut app).is_empty());

        send(
            &mut app,
            AndroidInputEvent::Tap {
                target: AndroidUiTarget::ChatSettings,
            },
        );
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::SetChatFilterVisibility {
                channel: UiChatChannel::Guild,
                visible: false,
            }),
        );
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::ApplyChatSettings),
        );
        let applied = app.world().resource::<UiState>();
        assert_eq!(applied.panel, UiPanel::None);
        assert!(applied.chat_settings.filter_guild);
        assert!(applied.chat_settings.filter_trade);
        let effects = take_effects(&mut app);
        assert_eq!(effects.len(), 2);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            UiEffect::ApplyChatSettings { settings }
                if settings.filter_guild && settings.filter_trade
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            UiEffect::PersistChatSettings { settings }
                if settings.filter_guild && settings.filter_trade
        )));
        assert!(take_effects(&mut app).is_empty());
    }

    #[test]
    fn android_close_panel_semantic_and_back_are_shared_close_paths() {
        let mut app = in_game_app();
        send(
            &mut app,
            AndroidInputEvent::Tap {
                target: AndroidUiTarget::Options,
            },
        );
        send(&mut app, AndroidInputEvent::Semantic(UiAction::ClosePanel));
        assert_eq!(app.world().resource::<UiState>().panel, UiPanel::None);

        send(
            &mut app,
            AndroidInputEvent::Tap {
                target: AndroidUiTarget::ChatSettings,
            },
        );
        send(&mut app, AndroidInputEvent::Back);
        assert_eq!(app.world().resource::<UiState>().panel, UiPanel::None);
        assert!(take_effects(&mut app).is_empty());
    }
}
