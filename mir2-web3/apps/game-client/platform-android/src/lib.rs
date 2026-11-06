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
    AndroidNetwork, AndroidShellState, AndroidUiActionQueue,
};
use bevy::app::AppExit;
use bevy::prelude::*;
use gateway_bridge::{
    clear_bounded_inbound_transaction, drain_bounded_inbound_into_models,
    enqueue_game_shop_purchase, enqueue_security_request, enqueue_storage_request,
    AndroidGatewayHostAdapter, AndroidGatewayHostWriteOutcome, AndroidGatewayHostWriteResult,
    AndroidGatewayInboundQueue, AndroidGatewayOutboundLease, AndroidGatewayOutboundQueue,
};
use mir2_bevy_runtime::{build_runtime_app, RuntimeWindowSpec};
use mir2_ui_core::{effect::UiEffect, reducer::reduce, state::UiState};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, OnceLock};

const ANDROID_HOST_TRANSPORT_CAPACITY: usize = 256;

#[derive(Debug, Default)]
struct AndroidGatewayFfiState {
    active: bool,
    generation: u64,
    lost_generation: Option<u64>,
    outbound: VecDeque<AndroidGatewayOutboundLease>,
    in_flight: BTreeMap<u64, AndroidGatewayOutboundLease>,
    results: VecDeque<(AndroidGatewayOutboundLease, AndroidGatewayHostWriteResult)>,
}

static ANDROID_GATEWAY_FFI_STATE: OnceLock<Mutex<AndroidGatewayFfiState>> = OnceLock::new();

fn android_gateway_ffi_state() -> &'static Mutex<AndroidGatewayFfiState> {
    ANDROID_GATEWAY_FFI_STATE.get_or_init(|| Mutex::new(AndroidGatewayFfiState::default()))
}

fn try_publish_android_gateway_leases(
    observed_generation: u64,
    leases: Vec<AndroidGatewayOutboundLease>,
) -> bool {
    let mut state = android_gateway_ffi_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !state.active || state.generation != observed_generation {
        return false;
    }
    state.outbound.extend(leases);
    true
}

#[derive(Debug, Resource)]
pub struct AndroidGatewayTransportEnabled(pub bool);

impl Default for AndroidGatewayTransportEnabled {
    fn default() -> Self {
        Self(cfg!(target_os = "android"))
    }
}

#[derive(Debug, Default, Resource)]
struct AndroidGatewayTransportLifecycle {
    last_network: AndroidNetwork,
}

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
            .init_resource::<AndroidGatewayTransportEnabled>()
            .init_resource::<AndroidGatewayTransportLifecycle>()
            .init_resource::<AndroidGatewayHostAdapter>()
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
                    drive_android_gateway_host_transport,
                    drain_android_gateway_inbound,
                    route_android_input_messages,
                    apply_queued_ui_actions,
                )
                    .chain(),
            );
    }
}

/// Production Activity/JNI/WebSocket host entry point.
///
/// The host calls this after the Android lifecycle/network state has been
/// delivered to the Bevy app. Each returned lease must be written by the host
/// and passed to [`report_android_gateway_write_result`] exactly once.
pub fn drain_android_gateway_for_host(
    app: &mut App,
    max_entries: usize,
) -> Vec<AndroidGatewayOutboundLease> {
    app.world_mut()
        .resource_scope(|world, mut adapter: Mut<AndroidGatewayHostAdapter>| {
            let shell = *world.resource::<AndroidShellState>();
            world.resource_scope(|_world, mut queue: Mut<AndroidGatewayOutboundQueue>| {
                adapter.drain_ready(&mut queue, &shell, max_entries)
            })
        })
}

/// Production host callback for the actual socket/JNI write result.
///
/// A failed Storage V2 write is terminal for that exact request: the pending
/// state becomes unknown and is cleared, the drained command is not replayed,
/// and the next request receives a fresh process-lifetime ID. Non-storage
/// failures are intentionally left unchanged by this callback.
pub fn report_android_gateway_write_result(
    app: &mut App,
    lease: AndroidGatewayOutboundLease,
    result: AndroidGatewayHostWriteResult,
) -> AndroidGatewayHostWriteOutcome {
    app.world_mut()
        .resource_scope(|world, mut adapter: Mut<AndroidGatewayHostAdapter>| {
            world.resource_scope(|world, mut queue: Mut<AndroidGatewayOutboundQueue>| {
                world.resource_scope(|_world, mut ui_state: Mut<UiState>| {
                    adapter.on_host_write_result(&mut queue, &mut ui_state, lease, result)
                })
            })
        })
}

/// Activate the production GameActivity/JNI transport bridge. The Android
/// host calls this exported C ABI symbol once its WebSocket writer is ready.
#[unsafe(no_mangle)]
pub extern "C" fn mir2_android_gateway_host_start() {
    let mut state = android_gateway_ffi_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let replaces_live_transport = state.active
        || !state.outbound.is_empty()
        || !state.in_flight.is_empty()
        || !state.results.is_empty();
    if replaces_live_transport {
        state.lost_generation = Some(state.generation);
    }
    let Some(generation) = state.generation.checked_add(1) else {
        state.active = false;
        return;
    };
    state.generation = generation;
    state.active = true;
    state.outbound.clear();
    state.in_flight.clear();
    state.results.clear();
}

/// Stop the production host transport. The next Bevy update performs the same
/// commit-unknown transition as a socket close and never replays a mutation.
#[unsafe(no_mangle)]
pub extern "C" fn mir2_android_gateway_host_stop() {
    let mut state = android_gateway_ffi_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.active = false;
    state.lost_generation = Some(state.generation);
    state.outbound.clear();
    state.in_flight.clear();
    state.results.clear();
}

/// Notify the production bridge that the WebSocket closed or connectivity was
/// lost after a write. Sent-but-unacknowledged Storage V2 operations become
/// unknown on the next Bevy update and are never replayed.
#[unsafe(no_mangle)]
pub extern "C" fn mir2_android_gateway_connection_lost() {
    let mut state = android_gateway_ffi_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.active = false;
    state.lost_generation = Some(state.generation);
    state.outbound.clear();
    state.in_flight.clear();
    state.results.clear();
}

/// Copy the next outbound JSON envelope into a caller-owned JNI/native buffer.
///
/// The envelope is `{ "sequence": u64, "command": <BrowserCommand> }`.
/// A null/undersized buffer returns the required byte length without consuming
/// the lease. A large enough buffer copies exactly that many UTF-8 bytes and
/// moves the lease to the bounded in-flight table. Zero means no message and
/// `-1` means the bridge rejected the request.
///
/// # Safety
///
/// When `buffer` is non-null, it must be writable for at least `capacity`
/// bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mir2_android_gateway_copy_next_outbound(
    buffer: *mut u8,
    capacity: usize,
) -> isize {
    let mut state = android_gateway_ffi_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !state.active {
        return 0;
    }
    let Some(lease) = state.outbound.front() else {
        return 0;
    };
    let command = match serde_json::from_str::<serde_json::Value>(&lease.outbound().json) {
        Ok(command) => command,
        Err(_) => return -1,
    };
    let encoded = match serde_json::to_vec(&json!({
        "sequence": lease.sequence(),
        "command": command,
    })) {
        Ok(encoded) => encoded,
        Err(_) => return -1,
    };
    let Ok(required) = isize::try_from(encoded.len()) else {
        return -1;
    };
    if buffer.is_null() || capacity < encoded.len() {
        return required;
    }
    // SAFETY: the caller contract above guarantees a writable buffer of at
    // least `capacity` bytes, and this branch verified `capacity >= len`.
    unsafe {
        std::ptr::copy_nonoverlapping(encoded.as_ptr(), buffer, encoded.len());
    }
    let lease = state.outbound.pop_front().expect("front was checked");
    if state.in_flight.insert(lease.sequence(), lease).is_some() {
        state.active = false;
        state.lost_generation = Some(state.generation);
        return -1;
    }
    required
}

/// Return the actual JNI/WebSocket write result for one copied lease.
#[unsafe(no_mangle)]
pub extern "C" fn mir2_android_gateway_report_write_result(sequence: u64, sent: bool) -> bool {
    let mut state = android_gateway_ffi_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(lease) = state.in_flight.remove(&sequence) else {
        return false;
    };
    if state.results.len() >= ANDROID_HOST_TRANSPORT_CAPACITY {
        state.active = false;
        state.lost_generation = Some(state.generation);
        state.outbound.clear();
        state.in_flight.clear();
        state.results.clear();
        return false;
    }
    state.results.push_back((
        lease,
        if sent {
            AndroidGatewayHostWriteResult::Sent
        } else {
            AndroidGatewayHostWriteResult::Failed
        },
    ));
    true
}

fn drive_android_gateway_host_transport(
    enabled: Res<AndroidGatewayTransportEnabled>,
    shell: Res<AndroidShellState>,
    mut lifecycle: ResMut<AndroidGatewayTransportLifecycle>,
    mut adapter: ResMut<AndroidGatewayHostAdapter>,
    mut queue: ResMut<AndroidGatewayOutboundQueue>,
    mut inbound: ResMut<AndroidGatewayInboundQueue>,
    mut ui_state: ResMut<UiState>,
) {
    if !enabled.0 {
        lifecycle.last_network = shell.network;
        return;
    }

    let network_lost = shell.network == AndroidNetwork::Unavailable
        && lifecycle.last_network != AndroidNetwork::Unavailable;
    lifecycle.last_network = shell.network;

    let (active, observed_generation, lost_generation, available, results) = {
        let mut state = android_gateway_ffi_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = state.active;
        let observed_generation = state.generation;
        let lost_generation = state.lost_generation.take();
        let available = ANDROID_HOST_TRANSPORT_CAPACITY.saturating_sub(state.outbound.len());
        let results = state.results.drain(..).collect::<Vec<_>>();
        (
            active,
            observed_generation,
            lost_generation,
            available,
            results,
        )
    };

    if network_lost || lost_generation.is_some() {
        let mut state = android_gateway_ffi_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let closes_current_transport = network_lost
            || lost_generation.is_some_and(|lost_generation| state.generation <= lost_generation);
        if closes_current_transport {
            state.active = false;
            state.outbound.clear();
            state.in_flight.clear();
            state.results.clear();
        }
        drop(state);
        adapter.on_connection_lost(&mut queue, &mut ui_state);
        clear_bounded_inbound_transaction(&mut inbound);
    }
    for (lease, result) in results {
        let _ = adapter.on_host_write_result(&mut queue, &mut ui_state, lease, result);
    }

    if !active || shell.network != AndroidNetwork::Available || available == 0 {
        return;
    }
    let leases = adapter.drain_ready(&mut queue, &shell, available);
    if leases.is_empty() {
        return;
    }
    if !try_publish_android_gateway_leases(observed_generation, leases) {
        adapter.on_connection_lost(&mut queue, &mut ui_state);
        clear_bounded_inbound_transaction(&mut inbound);
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
            // Clear possibly lost native transactions before enqueueing the
            // explicit logout command. The command itself is still retained.
            gateway.mark_terminal_reset();
            ui_state.mark_game_shop_unknown();
            ui_state.mark_storage_unknown();
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
        if let Some((operation, from, to)) = match &action {
            mir2_ui_core::action::UiAction::StoreItem { from, to } => Some((
                mir2_ui_core::storage::StorageOperation::StoreItem,
                *from,
                *to,
            )),
            mir2_ui_core::action::UiAction::TakeBackItem { from, to } => Some((
                mir2_ui_core::storage::StorageOperation::TakeBackItem,
                *from,
                *to,
            )),
            _ => None,
        } {
            let _ = enqueue_storage_request(
                &mut ui_state,
                &mut gateway,
                &mut inbound,
                operation,
                from,
                to,
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
    use crate::android_input::{
        AndroidInputEvent, AndroidLifecycle, AndroidLifecycleEvent, AndroidNetwork, AndroidUiTarget,
    };
    use mir2_ui_core::{
        action::UiAction,
        effect::UiEffect,
        state::{UiChatChannel, UiPanel, UiScreen, UiWindowMode},
    };

    static ANDROID_GATEWAY_FFI_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_android_gateway_ffi_for_test() {
        *android_gateway_ffi_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = AndroidGatewayFfiState::default();
    }

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

    fn make_host_ready(app: &mut App) {
        let mut shell = app.world_mut().resource_mut::<AndroidShellState>();
        shell.lifecycle = AndroidLifecycle::Foreground;
        shell.network = AndroidNetwork::Available;
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
    fn production_host_write_failure_marks_storage_unknown_and_next_request_is_new() {
        let mut app = in_game_app();
        make_host_ready(&mut app);
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::StoreItem { from: 3, to: 9 }),
        );

        let first_id = app
            .world()
            .resource::<UiState>()
            .storage_pending
            .as_ref()
            .expect("storage request is pending")
            .request_id
            .clone();
        let first_lease = drain_android_gateway_for_host(&mut app, 1)
            .into_iter()
            .next()
            .expect("host receives one storage lease");
        let late_duplicate = first_lease.clone();
        assert_eq!(first_lease.sequence(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&first_lease.outbound().json)
                .unwrap()
                .get("requestId")
                .and_then(serde_json::Value::as_str),
            Some(first_id.as_str())
        );

        assert_eq!(
            report_android_gateway_write_result(
                &mut app,
                first_lease,
                AndroidGatewayHostWriteResult::Failed,
            ),
            AndroidGatewayHostWriteOutcome::StorageMarkedUnknown
        );
        assert!(app.world().resource::<UiState>().storage_pending.is_none());
        assert!(app.world().resource::<UiState>().storage_unknown);
        assert!(drain_android_gateway_for_host(&mut app, 1).is_empty());

        // The same coordinates are intentionally reused. A late callback for
        // the drained old lease cannot release this new request.
        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::StoreItem { from: 3, to: 9 }),
        );
        let second_id = app
            .world()
            .resource::<UiState>()
            .storage_pending
            .as_ref()
            .expect("next storage request is pending")
            .request_id
            .clone();
        assert_ne!(first_id, second_id);
        let second_lease = drain_android_gateway_for_host(&mut app, 1)
            .into_iter()
            .next()
            .expect("host receives the next storage lease");
        assert_eq!(second_lease.sequence(), 2);
        assert_eq!(
            report_android_gateway_write_result(
                &mut app,
                late_duplicate,
                AndroidGatewayHostWriteResult::Failed,
            ),
            AndroidGatewayHostWriteOutcome::UnknownLease
        );
        assert!(app.world().resource::<UiState>().storage_pending.is_some());
        assert_eq!(
            report_android_gateway_write_result(
                &mut app,
                second_lease,
                AndroidGatewayHostWriteResult::Sent,
            ),
            AndroidGatewayHostWriteOutcome::Sent
        );
    }

    #[test]
    fn production_ffi_sent_without_ack_then_disconnect_allows_fresh_storage_id() {
        let _ffi_guard = ANDROID_GATEWAY_FFI_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_android_gateway_ffi_for_test();
        let mut app = in_game_app();
        app.world_mut()
            .resource_mut::<AndroidGatewayTransportEnabled>()
            .0 = true;
        make_host_ready(&mut app);
        mir2_android_gateway_host_start();

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::StoreItem { from: 3, to: 9 }),
        );
        app.update();
        let first_id = app
            .world()
            .resource::<UiState>()
            .storage_pending
            .as_ref()
            .expect("first storage request remains pending")
            .request_id
            .clone();

        let required = unsafe { mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) };
        assert!(required > 0);
        let mut bytes = vec![0_u8; required as usize];
        assert_eq!(
            unsafe { mir2_android_gateway_copy_next_outbound(bytes.as_mut_ptr(), bytes.len()) },
            required
        );
        let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let sequence = envelope["sequence"].as_u64().unwrap();
        assert_eq!(
            envelope["command"]["requestId"].as_str(),
            Some(first_id.as_str())
        );
        assert!(mir2_android_gateway_report_write_result(sequence, true));
        app.update();
        assert!(app.world().resource::<UiState>().storage_pending.is_some());

        mir2_android_gateway_connection_lost();
        // Reconnect before ECS consumes the loss flag. The barrier must remain
        // sticky and close the old sent-but-unacknowledged request first.
        mir2_android_gateway_host_start();
        app.update();
        assert!(app.world().resource::<UiState>().storage_pending.is_none());
        assert!(app.world().resource::<UiState>().storage_unknown);
        assert!(!mir2_android_gateway_report_write_result(sequence, false));
        assert_eq!(
            unsafe { mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) },
            0,
            "replacement transport cannot expose a stale lease for replay"
        );

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::StoreItem { from: 3, to: 9 }),
        );
        app.update();
        let second_id = app
            .world()
            .resource::<UiState>()
            .storage_pending
            .as_ref()
            .expect("new request is allowed after response loss")
            .request_id
            .clone();
        assert_ne!(first_id, second_id);

        let required = unsafe { mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) };
        assert!(
            required > 0,
            "replacement transport stays live without a second start"
        );
        let mut bytes = vec![0_u8; required as usize];
        assert_eq!(
            unsafe { mir2_android_gateway_copy_next_outbound(bytes.as_mut_ptr(), bytes.len()) },
            required
        );
        let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let second_sequence = envelope["sequence"].as_u64().unwrap();
        assert_eq!(
            envelope["command"]["requestId"].as_str(),
            Some(second_id.as_str())
        );
        assert!(mir2_android_gateway_report_write_result(
            second_sequence,
            true
        ));
        app.update();
        assert!(app.world().resource::<UiState>().storage_pending.is_some());

        app.world_mut().write_message(AndroidLifecycleMessage(
            crate::android_input::AndroidLifecycleEvent::NetworkUnavailable,
        ));
        app.update();
        assert!(app.world().resource::<UiState>().storage_pending.is_none());
        assert!(app.world().resource::<UiState>().storage_unknown);

        mir2_android_gateway_host_stop();
        app.update();
    }

    #[test]
    fn production_ffi_rejects_leases_drained_before_generation_change() {
        let _ffi_guard = ANDROID_GATEWAY_FFI_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_android_gateway_ffi_for_test();
        let mut app = in_game_app();
        app.world_mut()
            .resource_mut::<AndroidGatewayTransportEnabled>()
            .0 = true;
        make_host_ready(&mut app);
        mir2_android_gateway_host_start();

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::StoreItem { from: 4, to: 10 }),
        );
        let first_id = app
            .world()
            .resource::<UiState>()
            .storage_pending
            .as_ref()
            .expect("old-generation request is pending")
            .request_id
            .clone();
        let observed_generation = android_gateway_ffi_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .generation;
        let old_leases = drain_android_gateway_for_host(&mut app, 1);
        assert_eq!(old_leases.len(), 1);

        // Deterministically model host_start landing between the driver's
        // transport snapshot and its second lock/mailbox publication.
        mir2_android_gateway_host_start();
        assert!(!try_publish_android_gateway_leases(
            observed_generation,
            old_leases
        ));
        assert_eq!(
            unsafe { mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) },
            0,
            "an old-generation lease must never enter the replacement mailbox"
        );

        app.update();
        assert!(app.world().resource::<UiState>().storage_pending.is_none());
        assert!(app.world().resource::<UiState>().storage_unknown);

        send(
            &mut app,
            AndroidInputEvent::Semantic(UiAction::StoreItem { from: 4, to: 10 }),
        );
        app.update();
        let second_id = app
            .world()
            .resource::<UiState>()
            .storage_pending
            .as_ref()
            .expect("replacement-generation request is pending")
            .request_id
            .clone();
        assert_ne!(first_id, second_id);
        let required = unsafe { mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) };
        assert!(required > 0);
        let mut bytes = vec![0_u8; required as usize];
        assert_eq!(
            unsafe { mir2_android_gateway_copy_next_outbound(bytes.as_mut_ptr(), bytes.len()) },
            required
        );
        let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            envelope["command"]["requestId"].as_str(),
            Some(second_id.as_str())
        );

        mir2_android_gateway_host_stop();
        app.update();
    }

    #[test]
    fn production_ffi_never_exposes_local_only_as_socket_command() {
        let _ffi_guard = ANDROID_GATEWAY_FFI_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reset_android_gateway_ffi_for_test();
        let mut app = in_game_app();
        app.world_mut()
            .resource_mut::<AndroidGatewayTransportEnabled>()
            .0 = true;
        make_host_ready(&mut app);
        mir2_android_gateway_host_start();
        app.world_mut()
            .resource_mut::<AndroidGatewayOutboundQueue>()
            .enqueue(mir2_ui_core::effect::GatewayCommand::RetryConnection)
            .unwrap();
        app.update();

        assert_eq!(
            unsafe { mir2_android_gateway_copy_next_outbound(std::ptr::null_mut(), 0) },
            0,
            "LocalOnly retryConnection must never become a BrowserCommand"
        );
        mir2_android_gateway_host_stop();
        app.update();
    }

    #[test]
    fn production_host_non_storage_failure_does_not_change_existing_state() {
        let mut app = App::new();
        app.add_plugins(AndroidShellPlugin);
        {
            let mut state = app.world_mut().resource_mut::<UiState>();
            state.screen = UiScreen::Login;
            state.login_account = "account".into();
            state.login_password = "password".into();
        }
        make_host_ready(&mut app);
        send(&mut app, AndroidInputEvent::Semantic(UiAction::Login));

        let lease = drain_android_gateway_for_host(&mut app, 1)
            .into_iter()
            .next()
            .expect("host receives login lease");
        assert_eq!(
            report_android_gateway_write_result(
                &mut app,
                lease,
                AndroidGatewayHostWriteResult::Failed,
            ),
            AndroidGatewayHostWriteOutcome::NonStorageFailureIgnored
        );
        assert_eq!(
            app.world().resource::<AndroidGatewayOutboundQueue>().len(),
            0
        );
    }

    #[test]
    fn shared_guild_storage_actions_reach_android_queue_through_semantic_input() {
        let mut app = in_game_app();
        for action in [
            UiAction::RequestGuildStorage,
            UiAction::GuildStorageGoldChange {
                change_type: 0,
                amount: 250,
            },
            UiAction::GuildStorageItemChange {
                change_type: 2,
                from: 4,
                to: 7,
            },
        ] {
            send(&mut app, AndroidInputEvent::Semantic(action));
        }

        let mut shell = crate::android_input::AndroidShellState::default();
        shell.lifecycle = crate::android_input::AndroidLifecycle::Foreground;
        shell.network = crate::android_input::AndroidNetwork::Available;
        let outbound = app
            .world_mut()
            .resource_mut::<gateway_bridge::AndroidGatewayOutboundQueue>()
            .drain_ready(&shell, 3);
        let wire = outbound
            .iter()
            .map(|entry| serde_json::from_str::<serde_json::Value>(&entry.json).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            wire,
            vec![
                serde_json::json!({"type":"guildStorageItemChange","changeType":3,"from":0,"to":0}),
                serde_json::json!({"type":"guildStorageGoldChange","changeType":0,"amount":250}),
                serde_json::json!({"type":"guildStorageItemChange","changeType":2,"from":4,"to":7}),
            ]
        );
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
