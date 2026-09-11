//! Android adapter for the SAME Crystal shell used by the Windows host.
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::{login::CrystalLoginAction, CrystalStageTransform},
    native_shell::{
        CharacterSummary, NativeGatewayEvent as Event, NativeShellModel,
        NativeShellScreen as Screen, NativeUiIntent as Intent, NativeUiIntentQueue, ShellNotice,
    },
    native_shell_ui::{Mir2NativeShellUiPlugin, NativeShellRoot},
};
use serde_json::{json, Value};
use std::{collections::VecDeque, sync::Mutex};

static INBOX: Mutex<VecDeque<Value>> = Mutex::new(VecDeque::new());
static OUTBOX: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

fn enqueue_host_event(queue: &mut VecDeque<Value>, text: &str) {
    // Snapshot JSON is escaped once inside the host envelope. Retain the small
    // limit for ordinary input events and bound aggregate snapshot memory too.
    let parsed = (text.len() <= 2 * 1024 * 1024 + 65536)
        .then(|| serde_json::from_str::<Value>(text).ok())
        .flatten();
    let valid = parsed.as_ref().is_some_and(|value| {
        let snapshot_bytes = value["worldSnapshot"].as_str().map_or(0, str::len);
        snapshot_bytes <= 1024 * 1024
            && text.len() <= 65536 + 2 * snapshot_bytes
            && queue.len() < 32
            && queue
                .iter()
                .map(|event| 65536 + event["worldSnapshot"].as_str().map_or(0, str::len))
                .sum::<usize>()
                + snapshot_bytes
                + 65536
                <= 8 * 1024 * 1024
    });
    if valid {
        queue.push_back(parsed.unwrap());
    } else {
        queue.clear();
        queue.push_back(json!({"phase":"DISCONNECTED","message":"Invalid or overflowing host event; reconnect"}));
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_mir2_web3_MainActivity_nativeEvent<'a>(
    mut env: jni::EnvUnowned<'a>,
    _: jni::objects::JClass<'a>,
    text: jni::objects::JString<'a>,
) {
    env.with_env(|_| -> Result<(), jni::errors::Error> {
        let text = text.to_string();
        let mut queue = INBOX.lock().unwrap_or_else(|e| e.into_inner());
        enqueue_host_event(&mut queue, &text);
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_mir2_web3_MainActivity_nativePoll<'a>(
    mut env: jni::EnvUnowned<'a>,
    _: jni::objects::JClass<'a>,
) -> jni::sys::jstring {
    env.with_env(|env| -> Result<_, jni::errors::Error> {
        let command = OUTBOX
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
            .unwrap_or_default();
        Ok(env.new_string(command)?.into_raw())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn send(value: Value) {
    let mut queue = OUTBOX.lock().unwrap_or_else(|e| e.into_inner());
    if queue.len() < 16 {
        queue.push_back(value.to_string());
    }
}

#[derive(Resource, Default)]
pub(crate) struct HostState {
    phase: String,
    world: Option<HostWorldPosition>,
    pending_world_request: Option<u64>,
    pending_render_request: Option<u64>,
    ime_bottom: f32,
    pub(crate) safe_right: f32,
    pub(crate) safe_top: f32,
    safe_left: f32,
    safe_bottom: f32,
    render_load_active: bool,
    deferred_render_load: Option<DeferredRenderLoad>,
}

#[derive(Debug)]
struct DeferredRenderLoad {
    scene: crate::world_projection::ProjectedScene,
    world_snapshot: String,
    request_id: u64,
}

#[cfg(target_os = "android")]
fn start_deferred_render_load(host: &mut HostState) {
    if host.render_load_active {
        return;
    }
    let Some(request) = host.deferred_render_load.take() else {
        return;
    };
    let DeferredRenderLoad {
        scene,
        world_snapshot,
        request_id,
    } = request;
    if crate::world_assets::request_packaged_map_atlas_load(
        scene.clone(),
        world_snapshot.clone(),
        request_id,
    ) {
        host.render_load_active = true;
    } else {
        host.deferred_render_load = Some(DeferredRenderLoad {
            scene,
            world_snapshot,
            request_id,
        });
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostWorldPosition {
    player_name: String,
    map_file_name: String,
    x: u32,
    y: u32,
}

fn host_world_position(value: &Value, screen: Screen) -> Option<HostWorldPosition> {
    if value["phase"] != "IN_GAME" || !matches!(screen, Screen::StartingGame | Screen::InGame) {
        return None;
    }
    let world: HostWorldPosition = serde_json::from_value(value["world"].clone()).ok()?;
    if [&world.player_name, &world.map_file_name]
        .iter()
        .any(|s| s.trim().is_empty() || s.chars().count() > 128)
        || world.x > i32::MAX as u32
        || world.y > i32::MAX as u32
    {
        return None;
    }
    Some(world)
}

fn begin_render_ready_scene_transition(model: &mut NativeShellModel) {
    if model.screen != Screen::InGame {
        return;
    }
    // Reuse the shared StartingGame surface as an input/render barrier. The
    // authenticated character remains selected and personal models survive the
    // scene reset; only the exact next map+entity NativeRenderReady receipt may
    // return the shell to InGame.
    model.screen = Screen::StartingGame;
    model.notice = Some(ShellNotice::info(
        "Loading authoritative map and character assets.",
    ));
}

#[derive(Resource, Default)]
struct EditorTouch(bool);

fn remember_editor_touch(
    mut touch: ResMut<EditorTouch>,
    mut held: Local<bool>,
    targets: Query<
        &Interaction,
        With<mir2_client_bevy::crystal_ui::overlays::NativeTextInputTarget>,
    >,
) {
    // Capture before Update rebuilds overlay children. Querying those children
    // in PostUpdate loses the press when the focused field did not change.
    let pressed = targets
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed);
    touch.0 = pressed && !*held;
    *held = pressed;
}

pub struct AndroidSharedShellPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AndroidStageFit;
impl Plugin for AndroidSharedShellPlugin {
    fn build(&self, app: &mut App) {
        let mut model = NativeShellModel::default();
        model.screen = Screen::Login;
        model.notice = Some(ShellNotice::info(
            "Configure an approved test Gateway before connecting.",
        ));
        app.insert_resource(model)
            .init_resource::<HostState>()
            .init_resource::<EditorTouch>()
            .add_plugins(Mir2NativeShellUiPlugin)
            .add_plugins((
                mir2_client_bevy::crystal_ui::minimap::Mir2CrystalMiniMapPlugin,
                mir2_client_bevy::crystal_ui::hud::Mir2CrystalHudPlugin,
                mir2_client_bevy::crystal_ui::chat::Mir2CrystalChatPlugin,
                mir2_client_bevy::crystal_ui::notice::Mir2CrystalNoticePlugin,
                mir2_client_bevy::quest_ui::Mir2QuestUiPlugin,
            ))
            .add_systems(
                PreUpdate,
                (
                    receive,
                    tick_scene_effects,
                    discard_inactive_player_commands,
                )
                    .chain()
                    .before(bevy::input::InputSystems),
            )
            .add_systems(
                PreUpdate,
                remember_editor_touch.after(bevy::ui::UiSystems::Focus),
            )
            .add_systems(
                PostUpdate,
                (
                    observe_world_receipt,
                    observe_render_receipt,
                    fit_stage.in_set(AndroidStageFit),
                    fit_mail_composer,
                    forward_intents,
                    forward_quest_ui_intents,
                    discard_inactive_player_commands,
                    keyboard,
                )
                    .chain()
                    .before(bevy::ui::UiSystems::Layout),
            );
        #[cfg(feature = "ui-preview")]
        crate::ui_preview::install(app);
        crate::entity_overlays::install(app);
        crate::ground_labels::install(app);
        crate::mobile_ui::install(app);
        crate::scene_effects::install(app);
    }
}

fn fit_stage(
    windows: Query<&Window>,
    host: Res<HostState>,
    model: Res<NativeShellModel>,
    player: Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    forms: crate::form_input::FormInput,
    mut scale: ResMut<UiScale>,
    belt: Res<mir2_client_bevy::crystal_ui::hud::CrystalBeltPresentation>,
    mut roots: Query<
        (
            &mut Node,
            Has<mir2_client_bevy::crystal_ui::minimap::CrystalMiniMapRoot>,
        ),
        (
            Or<(
                With<NativeShellRoot>,
                With<mir2_client_bevy::crystal_ui::hud::CrystalHudRoot>,
                With<mir2_client_bevy::crystal_ui::chat::CrystalChatRoot>,
                With<mir2_client_bevy::crystal_ui::notice::CrystalNoticeRoot>,
                With<mir2_client_bevy::crystal_ui::minimap::CrystalMiniMapRoot>,
                With<mir2_client_bevy::crystal_ui::overlays::OverlayRoot>,
                With<mir2_client_bevy::quest_ui::QuestUiRoot>,
                With<crate::entity_overlays::ActorOverlayRoot>,
                With<crate::entity_overlays::DamageOverlayRoot>,
                With<crate::ground_labels::GroundDropLabelRoot>,
            )>,
            Without<mir2_client_bevy::crystal_ui::hud::CrystalHudMiniMapLayer>,
            Without<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>,
        ),
    >,
    mut minimap_layers: Query<
        &mut Node,
        (
            With<mir2_client_bevy::crystal_ui::hud::CrystalHudMiniMapLayer>,
            Without<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>,
        ),
    >,
    mut belt_layers: Query<&mut Node, With<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let fit = CrystalStageTransform::fit(window.width(), window.height());
    // Keep field/button size stable while typing. Pan the shared stage to keep
    // its login panel above the keyboard instead of shrinking it to a thumbnail.
    let available = (window.height() - host.ime_bottom / window.scale_factor()).max(1.0);
    let panel = mir2_client_bevy::crystal_ui::spec::login::PANEL.rect;
    let top = if host.ime_bottom > 0.0 && model.screen == Screen::InGame {
        let field = forms
            .field(&player)
            .or_else(|| crate::text_input::player_field(&player));
        let editor_bottom = player_editor_bottom(field.map(|field| field.0), fit.scale);
        (available / fit.scale - editor_bottom).min(fit.offset_y / fit.scale)
    } else if host.ime_bottom > 0.0 {
        (available / (2.0 * fit.scale) - panel.top - panel.height * 0.5)
            .min(fit.offset_y / fit.scale)
    } else {
        fit.offset_y / fit.scale
    };
    scale.0 = fit.scale;
    let map_origin = minimap_edge_origin(
        window.width(),
        window.scale_factor(),
        fit.scale,
        host.safe_right,
        host.safe_top,
    );
    for (mut root, is_map_image) in &mut roots {
        root.left = px(if is_map_image {
            map_origin.x
        } else {
            fit.offset_x / fit.scale
        });
        root.top = px(if is_map_image { map_origin.y } else { top });
    }
    // The frame/actions are children of the centered HUD; the map image is
    // a separate root. Compensate the parent transform so they stay aligned,
    // even while the dialog stage pans for IME. Bevy hits the moved nodes.
    for mut layer in &mut minimap_layers {
        layer.left = px(map_origin.x - fit.offset_x / fit.scale);
        layer.top = px(map_origin.y - top);
    }
    let belt_origin = belt_edge_origin(
        fit,
        window.height(),
        host.safe_left / window.scale_factor(),
        host.safe_bottom / window.scale_factor(),
        belt.vertical,
    );
    for mut layer in &mut belt_layers {
        let offset = belt_origin
            .map(|origin| origin - Vec2::new(fit.offset_x / fit.scale, top))
            .unwrap_or(Vec2::ZERO);
        layer.left = px(offset.x);
        layer.top = px(offset.y);
        // Editing must not leave relocated item buttons active over the IME.
        // The user's shared visible/orientation preference is left unchanged.
        layer.display = if host.ime_bottom > 0.0 {
            Display::None
        } else {
            Display::Flex
        };
    }
}

// Keep the two text fields visible while IME is open. Temporarily collapse
// attachment/gold controls and move the SAME shared footer under the fields;
// do not pan the entire 444px panel and push the recipient off screen.
fn fit_mail_composer(
    host: Res<HostState>,
    scale: Res<UiScale>,
    mut details: Query<
        &mut Node,
        (
            With<mir2_client_bevy::crystal_ui::overlays::MailComposeDetails>,
            Without<mir2_client_bevy::crystal_ui::overlays::MailComposeFooter>,
            Without<Button>,
        ),
    >,
    mut footers: Query<
        (&mut Node, &Children),
        (
            With<mir2_client_bevy::crystal_ui::overlays::MailComposeFooter>,
            Without<Button>,
        ),
    >,
    mut buttons: Query<
        &mut Node,
        (
            Without<mir2_client_bevy::crystal_ui::overlays::MailComposeDetails>,
            Without<mir2_client_bevy::crystal_ui::overlays::MailComposeFooter>,
        ),
    >,
) {
    let editing = host.ime_bottom > 0.0;
    for mut node in &mut details {
        node.display = if editing {
            Display::None
        } else {
            Display::Flex
        };
    }
    for (mut node, children) in &mut footers {
        node.top = px(if editing { 110.0 - 408.0 } else { 0.0 });
        for child in children.iter() {
            if let Ok(mut button) = buttons.get_mut(child) {
                button.height = px(if editing {
                    (44.0 / scale.0.max(0.01)).max(28.0)
                } else {
                    28.0
                });
            }
        }
    }
}

fn belt_edge_origin(
    fit: CrystalStageTransform,
    height: f32,
    safe_left: f32,
    safe_bottom: f32,
    vertical: bool,
) -> Option<Vec2> {
    let frame = mir2_client_bevy::crystal_ui::hud::belt_frame_rect(vertical);
    let left = safe_left + 8.0;
    if left + frame.width * fit.scale + 8.0 > fit.offset_x {
        return None; // Keep the source layout when the side gutter cannot fit it.
    }
    Some(Vec2::new(
        left / fit.scale - frame.left,
        (height - safe_bottom - 8.0) / fit.scale - frame.top - frame.height,
    ))
}

fn minimap_edge_origin(width: f32, dpi: f32, scale: f32, safe_right: f32, safe_top: f32) -> Vec2 {
    Vec2::new(
        (width - safe_right / dpi - 8.0) / scale - 1024.0,
        (safe_top / dpi + 8.0) / scale,
    )
}

fn player_editor_bottom(field: Option<&str>, scale: f32) -> f32 {
    match field {
        Some("guild-notice") => {
            // Shared notice text starts at panel +61, in 9px type. Keep the
            // eight visible editor rows above the IME; the server-side notice
            // limit is deliberately larger and must not expand this viewport.
            mir2_client_bevy::crystal_ui::overlays::CRYSTAL_GUILD_PANEL_RECT.top
                + 61.0
                + 8.0 * 12.0
                + 8.0
        }
        Some("inventory-amount" | "guild-amount" | "trade-amount") => {
            let rect = mir2_client_bevy::crystal_ui::overlays::CRYSTAL_DELETE_AMOUNT_RECT;
            rect.top + rect.height + 8.0
        }
        // Mail panel top + compact footer top + touch target + IME gutter.
        Some("mail-recipient" | "mail-message") => {
            5.0 + 110.0 + (44.0 / scale.max(0.01)).max(28.0) + 8.0 / scale.max(0.01)
        }
        _ => 750.0,
    }
}

fn enqueue_gateway_receipt(
    inbound: &mut crate::gateway_bridge::AndroidGatewayInboundQueue,
    raw: &str,
) -> bool {
    let Ok(envelope) = serde_json::from_str::<Value>(raw) else {
        return false;
    };
    match (
        envelope.get("type").and_then(Value::as_str),
        envelope.get("packet").and_then(Value::as_str),
    ) {
        (Some("gameShopReceipt"), _) => {
            crate::gateway_bridge::enqueue_native_game_shop_receipt(inbound, raw).is_ok()
        }
        (Some("packet"), Some("StoreItemV2" | "TakeBackItemV2")) => {
            crate::gateway_bridge::enqueue_native_storage_receipt(inbound, raw).is_ok()
        }
        (Some("packet"), Some("ChangePassword" | "ChangePasswordBanned")) => {
            crate::gateway_bridge::enqueue_native_change_password_result(inbound, raw).is_ok()
        }
        _ => false,
    }
}

fn receive(
    mut model: ResMut<NativeShellModel>,
    mut host: ResMut<HostState>,
    #[cfg_attr(not(target_os = "android"), allow(unused_variables))] time: Option<Res<Time>>,
    mut effects: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>>,
    mut intents: ResMut<NativeUiIntentQueue>,
    mut player: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
    mut key_events: Option<ResMut<Messages<bevy::input::keyboard::KeyboardInput>>>,
    windows: Query<Entity, With<Window>>,
    mut forms: crate::form_input::FormInput,
    mut lifecycle_messages: Option<ResMut<Messages<crate::android_input::AndroidLifecycleMessage>>>,
    mut gateway_inbound: Option<ResMut<crate::gateway_bridge::AndroidGatewayInboundQueue>>,
    mut ground_labels: Option<ResMut<crate::ground_labels::GroundDropLabelModel>>,
    mut actor_overlays: Option<ResMut<crate::entity_overlays::ActorOverlayModel>>,
    mut scene_effects: Option<ResMut<crate::scene_effects::SceneEffects>>,
    mut ground_pickups: Option<ResMut<mir2_client_bevy::quest_model::GroundPickupModel>>,
    #[cfg(feature = "ui-preview")] mut preview: ResMut<crate::ui_preview::PreviewRequest>,
) {
    #[cfg(target_os = "android")]
    if let Some(render) = crate::live_entity::poll_action_frame() {
        let _ = mir2_bevy_runtime::native_ingest::push_native_entity_render_state(render);
    }
    #[cfg(target_os = "android")]
    if let Some(event) = crate::world_assets::poll_packaged_map_atlas_load() {
        host.render_load_active = false;
        match event {
            crate::world_assets::PackagedMapAtlasLoadEvent::Ready(summary) => {
                info!(
                    pages = summary.page_count,
                    sprites = summary.source_count,
                    compressed_bytes = summary.compressed_bytes,
                    rgba_bytes = summary.rgba_bytes,
                    map_object_rgba_bytes = summary.map_object_rgba_bytes,
                    map_width = summary.map_width,
                    map_height = summary.map_height,
                    map_atlases = summary.map_atlas_count,
                    map_tiles = summary.map_tile_count,
                    map_standalone_tiles = summary.map_standalone_tile_count,
                    unresolved_draws = summary.unresolved_draw_count,
                    entity_pages = summary.entity_page_count,
                    entities = summary.entity_count,
                    entity_layers = summary.entity_layer_count,
                    unresolved_entities = summary.unresolved_entity_count,
                    entity_compressed_bytes = summary.entity_compressed_bytes,
                    entity_rgba_bytes = summary.entity_rgba_bytes,
                    "packaged Android world frame queued"
                );
                if matches!(model.screen, Screen::StartingGame | Screen::InGame) {
                    model.notice = Some(ShellNotice::info(format!(
                        "World frame queued: {} map tiles and {} entity layers ({} unresolved map draws, {} unresolved entities).",
                        summary.map_tile_count + summary.map_standalone_tile_count,
                        summary.entity_layer_count,
                        summary.unresolved_draw_count,
                        summary.unresolved_entity_count,
                    )));
                }
            }
            crate::world_assets::PackagedMapAtlasLoadEvent::Failed(message) => {
                warn!(%message, "packaged Android map atlas load failed");
                if matches!(model.screen, Screen::StartingGame | Screen::InGame) {
                    model.notice = Some(ShellNotice::error(format!(
                        "Packaged map atlas unavailable: {message}"
                    )));
                }
            }
        }
    }
    let values: Vec<_> = INBOX
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect();
    for value in values {
        if value["type"] == "gatewayGameplayPacket" {
            #[cfg(target_os = "android")]
            if let Some(raw) = value["envelope"].as_str() {
                let now_ms = time
                    .as_deref()
                    .map(|time| u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX))
                    .unwrap_or_default();
                let actor_positions = actor_overlays
                    .as_deref()
                    .map(crate::entity_overlays::ActorOverlayModel::actor_positions)
                    .unwrap_or_default();
                let effect_rejected = scene_effects.as_deref_mut().is_some_and(|effects| {
                    effects.observe_packet(raw, now_ms, &actor_positions)
                        == crate::scene_effects::EffectPacketOutcome::Rejected
                });
                if effect_rejected {
                    crate::live_entity::clear();
                    if let Some(effects) = scene_effects.as_deref_mut() {
                        effects.clear();
                    }
                    if let Some(overlays) = actor_overlays.as_deref_mut() {
                        overlays.reset();
                    }
                    if let Some(labels) = ground_labels.as_deref_mut() {
                        labels.reset();
                    }
                    if let Some(pickups) = ground_pickups.as_deref_mut() {
                        pickups.reset();
                    }
                    mir2_bevy_runtime::native_ingest::push_native_data_reset();
                    model.apply_gateway_event(Event::Disconnect {
                        reason: Some("Invalid authoritative scene-effect packet; reconnect".into()),
                    });
                    intents.drain().for_each(drop);
                    OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
                    send(json!({"type":"disconnect"}));
                    continue;
                }
                match crate::live_entity::apply_packet(raw) {
                    crate::live_entity::LiveEntityPacketOutcome::Applied { models, render } => {
                        let damage_events = crate::live_entity::drain_damage_events();
                        if let Some(overlays) = actor_overlays.as_deref_mut() {
                            let now_ms = time
                                .as_deref()
                                .map(|time| {
                                    u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX)
                                })
                                .unwrap_or_default();
                            overlays.observe_damage_events(damage_events, now_ms);
                            let projected = overlays.center().and_then(|(center_x, center_y)| {
                                crate::entity_overlays::project(&models, center_x, center_y)
                            });
                            if let Some(projected) = projected {
                                overlays.replace(projected);
                            } else {
                                overlays.reset();
                            }
                        }
                        if let Some(labels) = ground_labels.as_deref_mut() {
                            let projected = labels.center().and_then(|(center_x, center_y)| {
                                crate::ground_labels::project(&models, center_x, center_y)
                            });
                            if let Some(projected) = projected {
                                labels.replace(projected);
                            } else {
                                labels.reset();
                            }
                        }
                        if let Some(pickups) = ground_pickups.as_deref_mut() {
                            *pickups = crate::ground_pickups::project(&models).unwrap_or_default();
                        }
                        let _ =
                            mir2_bevy_runtime::native_ingest::push_native_entity_model_set(models);
                        if let Some(render) = render {
                            let _ =
                                mir2_bevy_runtime::native_ingest::push_native_entity_render_state(
                                    render,
                                );
                        }
                    }
                    crate::live_entity::LiveEntityPacketOutcome::Ignored => {}
                    crate::live_entity::LiveEntityPacketOutcome::Rejected => {
                        crate::live_entity::clear();
                        if let Some(effects) = scene_effects.as_deref_mut() {
                            effects.clear();
                        }
                        if let Some(overlays) = actor_overlays.as_deref_mut() {
                            overlays.reset();
                        }
                        if let Some(labels) = ground_labels.as_deref_mut() {
                            labels.reset();
                        }
                        if let Some(pickups) = ground_pickups.as_deref_mut() {
                            pickups.reset();
                        }
                        mir2_bevy_runtime::native_ingest::push_native_data_reset();
                        model.apply_gateway_event(Event::Disconnect {
                            reason: Some("Invalid authoritative entity packet; reconnect".into()),
                        });
                        intents.drain().for_each(drop);
                        OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        send(json!({"type":"disconnect"}));
                    }
                }
            }
            continue;
        }
        if value["type"] == "gatewayReceipt" {
            if let (Some(inbound), Some(raw)) =
                (gateway_inbound.as_deref_mut(), value["envelope"].as_str())
            {
                let _ = enqueue_gateway_receipt(inbound, raw);
            }
            continue;
        }
        if value["type"] == "lifecycle" {
            let event = match value["state"].as_str() {
                Some("resume") => Some(crate::android_input::AndroidLifecycleEvent::Resume),
                Some("pause") => Some(crate::android_input::AndroidLifecycleEvent::Pause),
                Some("destroy") => Some(crate::android_input::AndroidLifecycleEvent::Destroy),
                Some("networkAvailable") => {
                    Some(crate::android_input::AndroidLifecycleEvent::NetworkAvailable)
                }
                Some("networkUnavailable") => {
                    Some(crate::android_input::AndroidLifecycleEvent::NetworkUnavailable)
                }
                _ => None,
            };
            if let (Some(messages), Some(event)) = (lifecycle_messages.as_deref_mut(), event) {
                messages.write(crate::android_input::AndroidLifecycleMessage(event));
            }
            continue;
        }
        if value["type"] == "submit" {
            let active = crate::text_input::shell_field(&model)
                .map(|v| v.0)
                .or_else(|| {
                    player
                        .as_deref()
                        .filter(|_| model.screen == Screen::InGame)
                        .and_then(|p| {
                            forms
                                .field(p)
                                .or_else(|| crate::text_input::player_field(p))
                        })
                        .map(|v| v.0)
                });
            if active.is_some() && active == value["field"].as_str() {
                if let (Some(events), Ok(window)) = (key_events.as_deref_mut(), windows.single()) {
                    use bevy::input::{
                        keyboard::{Key, KeyboardInput},
                        ButtonState,
                    };
                    for state in [ButtonState::Pressed, ButtonState::Released] {
                        events.write(KeyboardInput {
                            key_code: KeyCode::Enter,
                            logical_key: Key::Enter,
                            state,
                            text: None,
                            repeat: false,
                            window,
                        });
                    }
                }
            }
            continue;
        }
        if value["type"] == "back" {
            match model.screen {
                Screen::InGame => {
                    if let (Some(events), Ok(window)) =
                        (key_events.as_deref_mut(), windows.single())
                    {
                        shared_back_key(events, window);
                    }
                }
                Screen::CharacterCreate => {
                    model.apply_ui_intent(Intent::CancelCharacterCreate);
                }
                Screen::ChangePassword => {
                    model.apply_ui_intent(Intent::CancelChangePassword);
                }
                Screen::SafeKey => {
                    model.apply_ui_intent(Intent::CloseSafeKey);
                }
                Screen::DeleteConfirm { .. } => {
                    model.apply_ui_intent(Intent::CancelDeleteCharacter);
                }
                _ => {}
            }
            continue;
        }
        #[cfg(feature = "ui-preview")]
        if value["type"] == "uiPreview" {
            if let Some(scene) = value["scene"]
                .as_str()
                .filter(|s| crate::ui_preview::SCENES.contains(s))
            {
                preview.scene = Some(scene.to_owned());
            }
            continue;
        }
        if value["type"] == "insets" {
            host.ime_bottom = value["bottom"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_right = value["safeRight"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_top = value["safeTop"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_left = value["safeLeft"].as_u64().unwrap_or(0).min(8192) as f32;
            host.safe_bottom = value["safeBottom"].as_u64().unwrap_or(0).min(8192) as f32;
            continue;
        }
        if value["type"] == "edit" {
            crate::text_input::edit_shell(
                &mut model,
                value["field"].as_str().unwrap_or(""),
                value["text"].as_str().unwrap_or(""),
            );
            if model.screen == Screen::InGame {
                if let Some(player) = player.as_deref_mut() {
                    forms.edit(
                        player,
                        value["field"].as_str().unwrap_or(""),
                        value["text"].as_str().unwrap_or(""),
                    );
                    crate::text_input::edit_player(
                        player,
                        value["field"].as_str().unwrap_or(""),
                        value["text"].as_str().unwrap_or(""),
                    );
                }
            }
            continue;
        }
        let phase = value["phase"].as_str().unwrap_or("");
        // Typed server data must not be recovered by parsing UI message text.
        // Reset on any non-world phase, including a map transition or reconnect.
        let next_world = host_world_position(&value, model.screen);
        let map_changed = host
            .world
            .as_ref()
            .zip(next_world.as_ref())
            .is_some_and(|(old, next)| old.map_file_name != next.map_file_name);
        host.world = next_world;
        if matches!(phase, "DISCONNECTED" | "UNCONFIGURED" | "CONNECTING") {
            host.pending_world_request = None;
            host.pending_render_request = None;
            host.render_load_active = false;
            host.deferred_render_load = None;
            #[cfg(target_os = "android")]
            crate::world_assets::cancel_packaged_map_atlas_load();
            if let Some(overlays) = actor_overlays.as_deref_mut() {
                overlays.reset();
            }
            if let Some(effects) = scene_effects.as_deref_mut() {
                effects.clear();
            }
            if let Some(labels) = ground_labels.as_deref_mut() {
                labels.reset();
            }
            if let Some(pickups) = ground_pickups.as_deref_mut() {
                pickups.reset();
            }
            mir2_bevy_runtime::native_ingest::push_native_data_reset();
        } else if map_changed || (phase == "STARTING" && host.phase == "IN_GAME") {
            begin_render_ready_scene_transition(&mut model);
            host.pending_world_request = None;
            host.pending_render_request = None;
            host.render_load_active = false;
            host.deferred_render_load = None;
            #[cfg(target_os = "android")]
            crate::world_assets::cancel_packaged_map_atlas_load();
            if let Some(overlays) = actor_overlays.as_deref_mut() {
                overlays.reset();
            }
            if let Some(effects) = scene_effects.as_deref_mut() {
                effects.clear();
            }
            if let Some(labels) = ground_labels.as_deref_mut() {
                labels.reset();
            }
            if let Some(pickups) = ground_pickups.as_deref_mut() {
                pickups.reset();
            }
            mir2_bevy_runtime::native_ingest::push_native_scene_reset();
        }
        if let (Some(world), Some(raw)) = (&host.world, value["worldSnapshot"].as_str()) {
            let projected = crate::world_projection::project(
                raw,
                &world.map_file_name,
                &world.player_name,
                world.x,
                world.y,
            );
            let mut projected_scene = None;
            let queued = projected.is_some_and(|projection| {
                let crate::world_projection::Projection {
                    request_id,
                    world,
                    ui,
                    map,
                    entities,
                    scene,
                } = projection;
                let projected_overlays =
                    crate::entity_overlays::project(&entities, scene.center_x, scene.center_y);
                let projected_labels =
                    crate::ground_labels::project(&entities, scene.center_x, scene.center_y);
                let projected_pickups = crate::ground_pickups::project(&entities);
                let world_snapshot = world.clone();
                host.pending_world_request = Some(request_id);
                host.pending_render_request = Some(request_id);
                #[cfg(target_os = "android")]
                let live_models_ready = crate::live_entity::install_models(&entities, request_id);
                #[cfg(not(target_os = "android"))]
                let live_models_ready = true;
                let queued = projected_overlays.is_some()
                    && projected_labels.is_some()
                    && projected_pickups.is_some()
                    && live_models_ready
                    && mir2_bevy_runtime::native_ingest::push_native_world_state(world)
                    && mir2_bevy_runtime::native_ingest::push_native_ui_read_model(ui)
                    && mir2_bevy_runtime::native_ingest::push_native_map_model(map)
                    && mir2_bevy_runtime::native_ingest::push_native_entity_model_set(entities);
                if queued {
                    if let (Some(overlays), Some(projected_overlays)) =
                        (actor_overlays.as_deref_mut(), projected_overlays)
                    {
                        overlays.replace(projected_overlays);
                    }
                    if let (Some(labels), Some(projected_labels)) =
                        (ground_labels.as_deref_mut(), projected_labels)
                    {
                        labels.replace(projected_labels);
                    }
                    if let (Some(pickups), Some(projected_pickups)) =
                        (ground_pickups.as_deref_mut(), projected_pickups)
                    {
                        *pickups = projected_pickups;
                    }
                    projected_scene = Some((scene, world_snapshot, request_id));
                }
                queued
            });
            if !queued {
                #[cfg(target_os = "android")]
                crate::world_assets::cancel_packaged_map_atlas_load();
                if let Some(overlays) = actor_overlays.as_deref_mut() {
                    overlays.reset();
                }
                if let Some(effects) = scene_effects.as_deref_mut() {
                    effects.clear();
                }
                if let Some(labels) = ground_labels.as_deref_mut() {
                    labels.reset();
                }
                if let Some(pickups) = ground_pickups.as_deref_mut() {
                    pickups.reset();
                }
                mir2_bevy_runtime::native_ingest::push_native_data_reset();
                host.world = None;
                host.pending_world_request = None;
                host.pending_render_request = None;
                host.render_load_active = false;
                host.deferred_render_load = None;
                host.phase = "DISCONNECTED".into();
                model.apply_gateway_event(Event::Disconnect {
                    reason: Some("World snapshot rejected; reconnect".into()),
                });
                intents.drain().for_each(drop);
                OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
                send(json!({"type":"disconnect"}));
                continue;
            }
            #[cfg(target_os = "android")]
            if let Some((scene, world_snapshot, request_id)) = projected_scene {
                // Keep only the newest authoritative frame while a previous
                // map/entity asset build is active. When it completes, the
                // next update starts this deferred frame; stale receipts cannot
                // unlock a transition because pending_render_request holds the
                // newest request id.
                host.deferred_render_load = Some(DeferredRenderLoad {
                    scene,
                    world_snapshot,
                    request_id,
                });
            }
        }
        if matches!(phase, "DISCONNECTED" | "UNCONFIGURED" | "CONNECTING") {
            if let Some(effects) = effects.as_deref_mut() {
                discard_player_commands(effects);
            }
        }
        let message = value["message"]
            .as_str()
            .unwrap_or("Connection unavailable")
            .to_owned();
        match phase {
            "UNCONFIGURED" => {
                model.apply_gateway_event(Event::Disconnect { reason: None });
                model.screen = Screen::Login;
                model.notice = Some(ShellNotice::error(message));
            }
            "CONNECTING" => {
                model.screen = Screen::Connecting;
            }
            "READY" if model.login_request_in_flight => {
                model.apply_gateway_event(Event::LoginFailure { message });
            }
            "READY" => {
                model.apply_gateway_event(Event::Connected);
            }
            "CHARACTERS" if model.login_request_in_flight => {
                let characters = value["characters"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|row| {
                        Some(CharacterSummary::new(
                            i32::try_from(row["index"].as_i64()?).ok()?,
                            row["name"].as_str()?,
                            row["level"].as_u64().unwrap_or(0).min(u16::MAX as u64) as u16,
                            row["className"].as_str().unwrap_or("Unknown"),
                            row["genderName"].as_str().unwrap_or("Unknown"),
                        ))
                    })
                    .collect();
                let account = model.login.account.clone();
                model.apply_gateway_event(Event::LoginSuccess {
                    account,
                    characters,
                });
            }
            "CHARACTERS" if model.screen == Screen::StartingGame => {
                model.apply_gateway_event(Event::StartGameAck {
                    accepted: false,
                    reason: Some(message),
                });
            }
            "IN_GAME" if model.screen == Screen::StartingGame => {
                model.apply_gateway_event(Event::StartGameAck {
                    accepted: true,
                    reason: None,
                });
                // Ingress is not bootstrap acceptance. Keep the loading screen
                // until the shared map/entity asset pipeline is actually ready.
                model.notice = Some(ShellNotice::info(message));
            }
            "DISCONNECTED" => {
                model.apply_gateway_event(Event::Disconnect {
                    reason: Some(message),
                });
                intents.drain().for_each(drop);
                OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
            }
            _ => {}
        }
        host.phase = phase.to_owned();
    }
    #[cfg(target_os = "android")]
    start_deferred_render_load(&mut host);
}

fn tick_scene_effects(
    time: Option<Res<Time>>,
    shell: Res<NativeShellModel>,
    player: Option<Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
    overlays: Option<Res<crate::entity_overlays::ActorOverlayModel>>,
    mut effects: Option<ResMut<crate::scene_effects::SceneEffects>>,
    #[cfg(feature = "ui-preview")] mut preview_effect_reported: Local<bool>,
) {
    let Some(effects) = effects.as_deref_mut() else {
        return;
    };
    let now_ms = time
        .as_deref()
        .map(|time| u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default();
    let center = overlays.as_deref().and_then(|overlays| overlays.center());
    let positions = overlays
        .as_deref()
        .map(crate::entity_overlays::ActorOverlayModel::actor_positions)
        .unwrap_or_default();
    let visible = shell.screen == Screen::InGame
        && player
            .as_deref()
            .is_none_or(|player| player.core.options.effect);
    if let Some(state) = effects.tick(now_ms, center, &positions, visible) {
        #[cfg(feature = "ui-preview")]
        let effect_count = serde_json::from_str::<Value>(&state)
            .ok()
            .and_then(|state| state["effects"].as_array().map(Vec::len))
            .unwrap_or_default();
        let accepted = mir2_bevy_runtime::native_ingest::push_native_effect_render_state(state);
        #[cfg(not(feature = "ui-preview"))]
        let _ = accepted;
        #[cfg(feature = "ui-preview")]
        if accepted && effect_count > 0 && !*preview_effect_reported {
            info!(effect_count, "ANDROID_EFFECT_RENDER_STATE_READY");
            *preview_effect_reported = true;
        }
    }
}

fn matching_world_receipt(
    pending: Option<u64>,
    receipt: &mir2_bevy_runtime::native_world_receipt::NativeWorldReceipt,
) -> Option<mir2_bevy_runtime::native_world_receipt::WorldApplyOutcome> {
    let (id, outcome) = receipt.last?;
    (pending == Some(id)).then_some(outcome)
}

fn render_receipt_matches_player(
    ready: &mir2_bevy_runtime::native_render_receipt::NativeRenderReady,
    world: Option<&HostWorldPosition>,
    character: &CharacterSummary,
) -> bool {
    // The scene center is an authoritative camera/render coordinate, not
    // necessarily the player's tile (for example, a server may clamp it near
    // map edges). The exact request id already binds this receipt to the
    // accepted snapshot, so only the authenticated player identity must match.
    ready.request_id != 0 && world.is_some_and(|world| world.player_name == character.name)
}

fn observe_render_receipt(
    mut host: ResMut<HostState>,
    mut model: ResMut<NativeShellModel>,
    receipt: Option<Res<mir2_bevy_runtime::native_render_receipt::NativeRenderReceipt>>,
) {
    let Some(request_id) = host.pending_render_request else {
        return;
    };
    let Some(ready) = receipt
        .as_deref()
        .and_then(|receipt| receipt.ready_for(request_id))
    else {
        return;
    };
    if model.screen == Screen::InGame {
        // Normal in-map snapshots refresh the renderer without replaying the
        // StartGame transition. Exact request matching above still prevents a
        // stale frame from clearing the newest pending update.
        host.pending_render_request = None;
        return;
    }
    if model.screen != Screen::StartingGame {
        return;
    }
    let Some(character) = model
        .selected_character_index
        .and_then(|selected| {
            model
                .characters
                .iter()
                .find(|character| character.index == selected)
        })
        .cloned()
    else {
        model.notice = Some(ShellNotice::error(
            "Render completed without a selected authenticated character.",
        ));
        return;
    };
    if !render_receipt_matches_player(&ready, host.world.as_ref(), &character) {
        model.notice = Some(ShellNotice::error(
            "Render receipt does not match the authenticated player.",
        ));
        return;
    }
    if model.apply_gateway_event(Event::PlayerBootstrapped { character }) {
        host.pending_render_request = None;
        model.notice = Some(ShellNotice::info(format!(
            "Entered game with {} map tiles and {} entity layers.",
            ready.map_tile_count, ready.entity_layer_count
        )));
    }
}

fn observe_world_receipt(
    mut host: ResMut<HostState>,
    mut model: ResMut<NativeShellModel>,
    receipt: Option<Res<mir2_bevy_runtime::native_world_receipt::NativeWorldReceipt>>,
    mut intents: ResMut<NativeUiIntentQueue>,
    mut effects: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>>,
    mut scene_effects: Option<ResMut<crate::scene_effects::SceneEffects>>,
) {
    use mir2_bevy_runtime::native_world_receipt::WorldApplyOutcome;
    let Some(outcome) = receipt
        .as_deref()
        .and_then(|receipt| matching_world_receipt(host.pending_world_request, receipt))
    else {
        return;
    };
    host.pending_world_request = None;
    match outcome {
        WorldApplyOutcome::Applied => {
            // Data acceptance is not map/entity render readiness or Bootstrap.
            if model.screen == Screen::StartingGame {
                model.notice = Some(ShellNotice::info(
                    "World data applied; waiting for map and character assets.",
                ));
            }
        }
        WorldApplyOutcome::DecodeRejected => {
            host.world = None;
            host.pending_render_request = None;
            host.render_load_active = false;
            host.deferred_render_load = None;
            host.phase = "DISCONNECTED".into();
            model.apply_gateway_event(Event::Disconnect {
                reason: Some("World data could not be decoded; reconnect".into()),
            });
            #[cfg(target_os = "android")]
            crate::world_assets::cancel_packaged_map_atlas_load();
            if let Some(scene_effects) = scene_effects.as_deref_mut() {
                scene_effects.clear();
            }
            mir2_bevy_runtime::native_ingest::push_native_data_reset();
            intents.drain().for_each(drop);
            if let Some(effects) = effects.as_deref_mut() {
                discard_player_commands(effects);
            }
            OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
            send(json!({"type":"disconnect"}));
        }
    }
}

// Android Back must follow the same modal priority and cancellation rules as
// desktop Escape, not clear parent windows and bypass the shared reducers.
fn shared_back_key(events: &mut Messages<bevy::input::keyboard::KeyboardInput>, window: Entity) {
    use bevy::input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    };
    for state in [ButtonState::Pressed, ButtonState::Released] {
        events.write(KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: Key::Escape,
            state,
            text: None,
            repeat: false,
            window,
        });
    }
}

fn forward_intents(
    mut model: ResMut<NativeShellModel>,
    host: Res<HostState>,
    mut intents: ResMut<NativeUiIntentQueue>,
    mut effects: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>>,
) {
    for intent in intents.drain() {
        match intent {
            Intent::Login | Intent::SafeKeyEnter if host.phase == "READY" => {
                send(
                    json!({"type":"login","account":model.login.account,"password":model.login.password}),
                );
                model.login.clear_password();
            }
            Intent::Login | Intent::SafeKeyEnter => {
                model.apply_gateway_event(Event::LoginFailure {
                    message: "Gateway not connected. Configure test endpoint and retry.".into(),
                });
                model.login.clear_password();
            }
            Intent::StartGame => {
                if let Some(index) = model.selected_character_index {
                    send(json!({"type":"start","index":index}));
                }
            }
            Intent::Retry => send(json!({"type":"connect"})),
            Intent::Logout => {
                if let Some(effects) = effects.as_deref_mut() {
                    discard_player_commands(effects);
                }
                send(json!({"type":"disconnect"}));
            }
            _ => {
                model.apply_gateway_event(Event::OperationFailure {
                    message: "This account operation is not wired in the Android UI milestone."
                        .into(),
                });
            }
        }
    }
}

/// Drain shared quest/object intents into Android's authenticated Gateway
/// producer. The authoritative snapshot remains unchanged until a server
/// packet or replacement snapshot arrives.
fn forward_quest_ui_intents(
    shell: Res<NativeShellModel>,
    windows: Query<&Window>,
    player: Option<Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
    dialog: Option<Res<mir2_client_bevy::quest_model::NpcDialogModel>>,
    read_model: Option<Res<mir2_client_bevy::read_model::UiReadModel>>,
    notice: Option<Res<mir2_client_bevy::crystal_ui::notice::NoticeDialogState>>,
    pickups: Option<Res<mir2_client_bevy::quest_model::GroundPickupModel>>,
    mut pending_operations: Option<ResMut<mir2_client_bevy::pending_operations::PendingOperations>>,
    mut quest_state: Option<ResMut<mir2_client_bevy::quest_ui::QuestUiState>>,
    mut intents: ResMut<mir2_client_bevy::quest_ui::QuestUiIntentQueue>,
    mut gateway: Option<ResMut<crate::gateway_bridge::AndroidGatewayOutboundQueue>>,
) {
    use mir2_client_bevy::quest_ui::QuestUiIntent;
    use mir2_ui_core::effect::GatewayCommand;

    let drained = intents.drain_intents();
    if drained.is_empty() {
        return;
    }
    let active =
        shell.screen == Screen::InGame && windows.single().is_ok_and(|window| window.focused);
    let dialog_open = dialog.as_deref().is_some_and(|dialog| dialog.is_open);
    let dead = read_model
        .as_deref()
        .is_some_and(|model| model.player.max_hp > 0 && model.player.hp <= 0);
    let world_actions_blocked = notice.as_deref().is_some_and(|notice| notice.is_open())
        || player
            .as_deref()
            .map(|player| player.blocks_world_action(dialog_open, dead))
            .unwrap_or(dialog_open || dead);
    let mut retry = Vec::new();

    for intent in drained {
        if !active {
            if let (Some(pending), Some(key)) =
                (pending_operations.as_deref_mut(), intent.pending_key())
            {
                pending.release(&key);
            }
            continue;
        }
        let command = match &intent {
            QuestUiIntent::InteractNpc { npc_object_id } if !world_actions_blocked => {
                Some(GatewayCommand::InteractNpc {
                    object_id: *npc_object_id,
                })
            }
            QuestUiIntent::SelectNpcDialog { target } => Some(GatewayCommand::SelectNpcDialog {
                target: target.clone(),
            }),
            QuestUiIntent::AcceptQuest {
                npc_index,
                quest_index,
            } => Some(GatewayCommand::AcceptQuest {
                npc_index: *npc_index,
                quest_index: *quest_index,
            }),
            QuestUiIntent::FinishQuest {
                quest_index,
                selected_item_index,
            } => Some(GatewayCommand::FinishQuest {
                quest_index: *quest_index,
                selected_item_index: *selected_item_index,
            }),
            QuestUiIntent::AbandonQuest { quest_index } => Some(GatewayCommand::AbandonQuest {
                quest_index: *quest_index,
            }),
            QuestUiIntent::AttackTarget { object_id } if !world_actions_blocked => {
                Some(GatewayCommand::AttackTarget {
                    object_id: *object_id,
                })
            }
            QuestUiIntent::PickUpObject { object_id }
                if !world_actions_blocked
                    && pickups.as_deref().is_some_and(|pickups| {
                        pickups
                            .recent
                            .iter()
                            .any(|pickup| pickup.object_id == Some(*object_id))
                    }) =>
            {
                Some(GatewayCommand::PickUp {
                    object_id: *object_id,
                })
            }
            QuestUiIntent::InteractNpc { .. }
            | QuestUiIntent::AttackTarget { .. }
            | QuestUiIntent::PickUpObject { .. } => None,
            QuestUiIntent::ShareQuest { .. } | QuestUiIntent::PickUpTile => {
                if let Some(quest_state) = quest_state.as_deref_mut() {
                    quest_state.set_feedback(
                        "This Android host action has no authenticated Gateway command yet",
                        true,
                    );
                }
                None
            }
        };
        let Some(command) = command else {
            if let (Some(pending), Some(key)) =
                (pending_operations.as_deref_mut(), intent.pending_key())
            {
                pending.release(&key);
            }
            continue;
        };
        let accepted = gateway
            .as_deref_mut()
            .is_some_and(|gateway| gateway.enqueue(command).is_ok());
        if !accepted {
            retry.push(intent);
        }
    }

    let dropped = intents.retain_failed_intents(retry);
    for intent in dropped {
        if let (Some(pending), Some(key)) =
            (pending_operations.as_deref_mut(), intent.pending_key())
        {
            pending.release(&key);
        }
    }
}

// Only unsent shared Gateway effects are invalidated. Local option persistence
// and application effects must survive; this is not a server rollback/receipt.
fn discard_player_commands(effects: &mut mir2_client_bevy::crystal_ui::overlays::UiEffectQueue) {
    let mut discarded = 0;
    for effect in effects.drain() {
        if matches!(effect, mir2_ui_core::effect::UiEffect::GatewayCommand(_)) {
            discarded += 1;
        } else {
            effects.push(effect);
        }
    }
    if discarded > 0 {
        debug!("Discarded {discarded} unsent shared player commands at Android session boundary");
        #[cfg(feature = "ui-preview")]
        info!("ANDROID_UI_PREVIEW_DISCARD count={discarded} unsent shared commands");
    }
}

fn discard_inactive_player_commands(
    model: Res<NativeShellModel>,
    windows: Query<&Window>,
    mut effects: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::UiEffectQueue>>,
) {
    let active =
        model.screen == Screen::InGame && windows.single().is_ok_and(|window| window.focused);
    if !active {
        if let Some(effects) = effects.as_deref_mut() {
            discard_player_commands(effects);
        }
    }
}

fn keyboard(
    model: Res<NativeShellModel>,
    player: Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    forms: crate::form_input::FormInput,
    editor_touch: Res<EditorTouch>,
    interactions: Query<(&Interaction, &CrystalLoginAction), Changed<Interaction>>,
    extra_fields: Query<
        (
            &Interaction,
            &mir2_client_bevy::native_shell_ui::NativeShellField,
        ),
        Changed<Interaction>,
    >,
    mut was_editing: Local<bool>,
    mut privacy: Local<bool>,
    mut last_field: Local<Option<String>>,
) {
    let sensitive = !model.login.account.is_empty()
        || !model.login.password.is_empty()
        || (model.screen == Screen::InGame && forms.field(&player).is_some_and(|v| v.2))
        || matches!(model.screen, Screen::ChangePassword | Screen::SafeKey);
    if *privacy != sensitive {
        send(json!({"type":"privacy","secure":sensitive}));
        *privacy = sensitive;
    }
    let field = crate::text_input::shell_field(&model).or_else(|| {
        (model.screen == Screen::InGame)
            .then(|| {
                forms
                    .field(&player)
                    .or_else(|| crate::text_input::player_field(&player))
            })
            .flatten()
    });
    let field_name = field.map(|v| v.0.to_owned());
    let pressed = editor_touch.0
        || interactions.iter().any(|(interaction, action)| {
            *interaction == Interaction::Pressed
                && matches!(
                    action,
                    CrystalLoginAction::FocusAccount | CrystalLoginAction::FocusPassword
                )
        })
        || extra_fields
            .iter()
            .any(|(interaction, _)| *interaction == Interaction::Pressed)
        || (model.screen == Screen::InGame && field_name.is_some() && field_name != *last_field);
    if pressed {
        if let Some((field, text, password)) = field {
            send(
                json!({"type":"keyboard","field":field,"text":text,"password":password,"numeric":field.ends_with("amount"),"multiline":crate::text_input::is_multiline_editor(field)}),
            );
            *was_editing = true;
        } else if *was_editing {
            send(json!({"type":"hideKeyboard"}));
            *was_editing = false;
        }
    } else if field.is_none() && *was_editing {
        send(json!({"type":"hideKeyboard"}));
        *was_editing = false;
    }
    *last_field = field_name;
}

#[cfg(test)]
mod tests {
    #[test]
    fn gateway_receipt_bridge_accepts_only_supported_authoritative_results() {
        let mut inbound = crate::gateway_bridge::AndroidGatewayInboundQueue::default();
        for raw in [
            r#"{"type":"gameShopReceipt","protocol":"nativeGameShopReceiptV1","requestId":"gs-1","success":true,"gIndex":31,"quantity":1,"priceType":1}"#,
            r#"{"type":"packet","packet":"StoreItemV2","payload":{"requestId":"st-1","from":3,"to":9,"success":true}}"#,
            r#"{"type":"packet","packet":"ChangePassword","payload":{"result":6}}"#,
        ] {
            assert!(super::enqueue_gateway_receipt(&mut inbound, raw));
        }
        assert_eq!(inbound.status().len, 3);
        assert!(!super::enqueue_gateway_receipt(
            &mut inbound,
            r#"{"type":"packet","packet":"ObjectChat","payload":{}}"#,
        ));
        assert!(!super::enqueue_gateway_receipt(&mut inbound, "not-json"));
        assert_eq!(inbound.status().len, 3);
    }

    #[test]
    fn render_receipt_accepts_an_authoritative_camera_center_away_from_player_tile() {
        use mir2_bevy_runtime::native_render_receipt::NativeRenderReady;
        let ready = NativeRenderReady {
            request_id: 42,
            center_x: 299,
            center_y: 630,
            map_tile_count: 607,
            entity_count: 2,
            entity_layer_count: 2,
        };
        let world = HostWorldPosition {
            player_name: "Fixture".into(),
            map_file_name: "0".into(),
            x: 302,
            y: 634,
        };
        let character = CharacterSummary::new(7, "Fixture", 12, "Wizard", "Female");
        assert!(super::render_receipt_matches_player(
            &ready,
            Some(&world),
            &character
        ));

        let other = CharacterSummary::new(8, "Other", 1, "Warrior", "Male");
        assert!(!super::render_receipt_matches_player(
            &ready,
            Some(&world),
            &other
        ));
        assert!(!super::render_receipt_matches_player(
            &ready, None, &character
        ));
    }

    #[test]
    fn authoritative_pickup_intent_reaches_android_gateway_without_local_removal() {
        use crate::{
            android_input::{AndroidLifecycle, AndroidNetwork, AndroidShellState},
            gateway_bridge::AndroidGatewayOutboundQueue,
        };
        use mir2_client_bevy::{
            crystal_ui::overlays::NativePlayerUiState,
            quest_model::{GroundPickupModel, RecentPickup},
            quest_ui::{QuestUiIntent, QuestUiIntentQueue},
        };

        let mut pickups = GroundPickupModel::default();
        pickups.upsert(RecentPickup {
            object_id: Some(44),
            key: "object:44".into(),
            label: "Red Potion".into(),
            amount: 2,
            from_npc: None,
        });
        let mut player = NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        let mut intents = QuestUiIntentQueue::default();
        assert!(intents.push_intent(QuestUiIntent::PickUpObject { object_id: 44 }));

        let mut app = App::new();
        app.insert_resource(NativeShellModel {
            screen: Screen::InGame,
            ..default()
        })
        .insert_resource(player)
        .insert_resource(pickups)
        .insert_resource(intents)
        .init_resource::<AndroidGatewayOutboundQueue>()
        .add_systems(Update, forward_quest_ui_intents);
        app.world_mut().spawn(Window {
            focused: true,
            ..default()
        });
        app.update();

        let mut transport = AndroidShellState::default();
        transport.lifecycle = AndroidLifecycle::Foreground;
        transport.network = AndroidNetwork::Available;
        let entries = app
            .world_mut()
            .resource_mut::<AndroidGatewayOutboundQueue>()
            .drain_ready(&transport, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            serde_json::from_str::<Value>(&entries[0].json).unwrap(),
            json!({"type":"pickUp","objectId":44})
        );
        assert_eq!(
            app.world()
                .resource::<GroundPickupModel>()
                .recent
                .front()
                .and_then(|pickup| pickup.object_id),
            Some(44)
        );
    }

    #[test]
    fn map_transition_reuses_loading_barrier_without_reauthentication() {
        let character = CharacterSummary::new(7, "Fixture", 12, "Wizard", "Female");
        let mut model = NativeShellModel {
            screen: Screen::InGame,
            characters: vec![character.clone()],
            selected_character_index: Some(character.index),
            active_character: Some(character.clone()),
            ..default()
        };

        super::begin_render_ready_scene_transition(&mut model);
        assert_eq!(model.screen, Screen::StartingGame);
        assert_eq!(model.selected_character_index, Some(character.index));
        assert_eq!(model.active_character.as_ref(), Some(&character));
        assert!(model.apply_gateway_event(Event::StartGameAck {
            accepted: true,
            reason: None,
        }));
        assert_eq!(model.screen, Screen::StartingGame);
        assert!(model.apply_gateway_event(Event::PlayerBootstrapped {
            character: character.clone(),
        }));
        assert_eq!(model.screen, Screen::InGame);
        assert_eq!(model.active_character.as_ref(), Some(&character));
    }

    #[test]
    fn world_receipt_requires_exact_request_and_does_not_unlock_gameplay() {
        use mir2_bevy_runtime::native_world_receipt::{NativeWorldReceipt, WorldApplyOutcome};
        let receipt = NativeWorldReceipt {
            last: Some((41, WorldApplyOutcome::Applied)),
        };
        assert_eq!(super::matching_world_receipt(None, &receipt), None);
        assert_eq!(super::matching_world_receipt(Some(42), &receipt), None);
        let mut app = App::new();
        let mut model = NativeShellModel::default();
        model.screen = Screen::StartingGame;
        app.insert_resource(model)
            .insert_resource(HostState {
                pending_world_request: Some(41),
                ..default()
            })
            .insert_resource(receipt)
            .init_resource::<NativeUiIntentQueue>()
            .add_systems(Update, observe_world_receipt);
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::StartingGame
        );
        assert!(app
            .world()
            .resource::<HostState>()
            .pending_world_request
            .is_none());
        let rejected = NativeWorldReceipt {
            last: Some((42, WorldApplyOutcome::DecodeRejected)),
        };
        assert_eq!(
            super::matching_world_receipt(Some(42), &rejected),
            Some(WorldApplyOutcome::DecodeRejected)
        );
        assert_eq!(super::matching_world_receipt(Some(43), &rejected), None);
    }

    #[test]
    fn host_inbox_accepts_large_snapshot_but_bounds_memory_and_fails_closed() {
        let mut queue = std::collections::VecDeque::new();
        let event =
            serde_json::json!({"phase":"IN_GAME", "worldSnapshot":"x".repeat(900_000)}).to_string();
        super::enqueue_host_event(&mut queue, &event);
        assert_eq!(queue[0]["worldSnapshot"].as_str().unwrap().len(), 900_000);
        for _ in 0..8 {
            super::enqueue_host_event(&mut queue, &event);
        }
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0]["phase"], "DISCONNECTED");
        super::enqueue_host_event(
            &mut queue,
            &serde_json::json!({"text":"x".repeat(65537)}).to_string(),
        );
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0]["phase"], "DISCONNECTED");
        super::enqueue_host_event(&mut queue, "not JSON");
        assert_eq!(queue[0]["phase"], "DISCONNECTED");
    }

    use super::*;

    #[test]
    fn structured_world_requires_active_start_and_never_parses_notice_text() {
        let event = json!({"phase":"IN_GAME", "world": {
            "playerName":"ServerPlayer", "mapFileName":"0", "x":302, "y":634
        }});
        let position = host_world_position(&event, Screen::StartingGame).unwrap();
        assert_eq!((position.x, position.y), (302, 634));
        assert_eq!(position.map_file_name, "0");
        assert!(host_world_position(&event, Screen::Login).is_none());
        assert!(host_world_position(
            &json!({"phase":"IN_GAME", "message":"Server position: (302,634)"}),
            Screen::StartingGame
        )
        .is_none());
        for phase in ["DISCONNECTED", "CONNECTING", "STARTING", "CHARACTERS"] {
            let mut changed = event.clone();
            changed["phase"] = json!(phase);
            assert!(host_world_position(&changed, Screen::StartingGame).is_none());
        }
        for x in [json!(-1), json!(1.5), json!("302"), json!(4294967295u64)] {
            let mut invalid = event.clone();
            invalid["world"]["x"] = x;
            assert!(host_world_position(&invalid, Screen::StartingGame).is_none());
        }
    }

    #[test]
    fn belt_edge_fits_side_gutter_and_falls_back_without_overlap() {
        let fit = CrystalStageTransform::fit(891.0, 411.0);
        for vertical in [false, true] {
            let frame = mir2_client_bevy::crystal_ui::hud::belt_frame_rect(vertical);
            let origin = belt_edge_origin(fit, 411.0, 0.0, 12.0, vertical).unwrap();
            assert!(((origin.x + frame.left) * fit.scale - 8.0).abs() < 0.001);
            assert!(((origin.y + frame.top + frame.height) * fit.scale - 391.0).abs() < 0.001);
            assert!((origin.x + frame.left + frame.width) * fit.scale < fit.offset_x);
        }
        assert!(belt_edge_origin(
            CrystalStageTransform::fit(1024.0, 768.0),
            768.0,
            0.0,
            0.0,
            false
        )
        .is_none());
        assert!(belt_edge_origin(fit, 411.0, 100.0, 0.0, false).is_none());
    }

    #[test]
    fn minimap_anchor_preserves_scale_and_safe_edge_across_aspects() {
        for (width, height, dpi, right, top) in [
            (1024.0, 768.0, 1.0, 0.0, 0.0),
            (891.0, 411.0, 2.625, 63.0, 24.0),
            (1280.0, 720.0, 1.0, 40.0, 32.0),
        ] {
            let fit = CrystalStageTransform::fit(width, height);
            let origin = minimap_edge_origin(width, dpi, fit.scale, right, top);
            assert!(((origin.x + 1024.0) * fit.scale - (width - right / dpi - 8.0)).abs() < 0.001);
            assert!((origin.y * fit.scale - (top / dpi + 8.0)).abs() < 0.001);
            // A translated group moves both visual and button geometry. No
            // independent window-cursor rewrite is applied to centered dialogs.
            let button_x = 903.0;
            let screen_x = (origin.x + button_x) * fit.scale;
            assert!((screen_x / fit.scale - origin.x - button_x).abs() < 0.001);
            for dialog_top in [fit.offset_y / fit.scale, -180.0] {
                let group_top = origin.y - dialog_top;
                assert!((dialog_top + group_top - origin.y).abs() < 0.001);
            }
        }
    }

    #[test]
    fn shared_session_reset_clears_android_player_intents_without_reusing_storage_ids() {
        use mir2_client_bevy::{
            crystal_ui::overlays::{
                MailComposeUi, NativePlayerUiIntent, NativePlayerUiIntentQueue,
                NativePlayerUiState, UiEffectQueue,
            },
            pending_operations::{
                apply_overlay_session_reset, observe_native_session_boundary,
                AuthoritativeModelRevisions, InventoryOperationFeedback,
                NativeSessionBoundaryTracker, OverlayResetTracker, PendingOperations,
                SessionResetGameShopPreservation, SessionResetRevision,
            },
        };
        for destination in [Screen::Login, Screen::ConnectionLost] {
            let mut app = App::new();
            app.insert_resource(NativeShellModel {
                screen: Screen::InGame,
                ..default()
            })
            .init_resource::<NativePlayerUiState>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<UiEffectQueue>()
            .init_resource::<MailComposeUi>()
            .init_resource::<PendingOperations>()
            .init_resource::<InventoryOperationFeedback>()
            .init_resource::<NativeSessionBoundaryTracker>()
            .init_resource::<OverlayResetTracker>()
            .init_resource::<AuthoritativeModelRevisions>()
            .init_resource::<SessionResetRevision>()
            .init_resource::<SessionResetGameShopPreservation>()
            // Match the shared plugin: reset runs before input/boundary observation.
            .add_systems(
                Update,
                (apply_overlay_session_reset, observe_native_session_boundary).chain(),
            );
            app.update();
            let first_id = app.world_mut().resource_scope(
                |world, mut queue: Mut<NativePlayerUiIntentQueue>| {
                    let mut pending = world.resource_mut::<PendingOperations>();
                    assert!(queue.push_storage_pending_intent(&mut pending, true, 7, 0, 1));
                    let queued = queue.drain_intents();
                    let NativePlayerUiIntent::StoreItem { request_id, .. } = &queued[0] else {
                        panic!("expected storage intent")
                    };
                    let id = request_id.clone();
                    for intent in queued {
                        assert!(queue.push_intent(intent));
                    }
                    assert!(queue.push_intent(NativePlayerUiIntent::Chat {
                        message: "old-session".into()
                    }));
                    id
                },
            );
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .chat_draft = "old draft".into();
            app.world_mut().resource_mut::<NativeShellModel>().screen = destination;
            app.update();
            assert_eq!(app.world().resource::<SessionResetRevision>().0, 1);
            app.update();
            assert!(app
                .world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents()
                .is_empty());
            assert!(app.world().resource::<PendingOperations>().is_empty());
            assert!(app
                .world()
                .resource::<NativePlayerUiState>()
                .chat_draft
                .is_empty());
            let next_id = app.world_mut().resource_scope(
                |world, mut queue: Mut<NativePlayerUiIntentQueue>| {
                    let mut pending = world.resource_mut::<PendingOperations>();
                    assert!(queue.push_storage_pending_intent(&mut pending, true, 8, 0, 1));
                    let queued = queue.drain_intents();
                    let NativePlayerUiIntent::StoreItem { request_id, .. } = &queued[0] else {
                        panic!("expected storage intent")
                    };
                    request_id.clone()
                },
            );
            assert_ne!(
                first_id, next_id,
                "session cleanup must not reset request identity"
            );
        }
    }

    #[test]
    fn invalidation_discards_network_effects_but_retains_local_effects_in_order() {
        use mir2_client_bevy::crystal_ui::overlays::UiEffectQueue;
        use mir2_ui_core::effect::{GatewayCommand, UiEffect};
        let mut effects = UiEffectQueue::default();
        effects.push(UiEffect::ExitApplication);
        effects.push(UiEffect::GatewayCommand(GatewayCommand::TownRevive));
        effects.push(UiEffect::ExitApplication);
        discard_player_commands(&mut effects);
        discard_player_commands(&mut effects);
        assert_eq!(
            effects.drain(),
            vec![UiEffect::ExitApplication, UiEffect::ExitApplication]
        );
    }

    #[test]
    fn inactive_cleanup_runs_before_and_after_ui_producers_and_never_replays_on_resume() {
        use mir2_client_bevy::crystal_ui::overlays::UiEffectQueue;
        use mir2_ui_core::effect::{GatewayCommand, UiEffect};
        fn produce(mut effects: ResMut<UiEffectQueue>) {
            effects.push(UiEffect::GatewayCommand(GatewayCommand::TownRevive));
        }
        for (in_game, focused, has_window) in [
            (false, true, true),
            (true, false, true),
            (true, true, false),
            (true, true, true),
        ] {
            let mut app = App::new();
            app.insert_resource(NativeShellModel {
                screen: if in_game {
                    Screen::InGame
                } else {
                    Screen::Login
                },
                ..default()
            })
            .init_resource::<UiEffectQueue>()
            .add_systems(PreUpdate, discard_inactive_player_commands)
            .add_systems(Update, produce)
            .add_systems(PostUpdate, discard_inactive_player_commands);
            if has_window {
                app.world_mut().spawn(Window {
                    focused,
                    ..default()
                });
            }
            app.world_mut()
                .resource_mut::<UiEffectQueue>()
                .push(UiEffect::GatewayCommand(GatewayCommand::TownRevive));
            app.update();
            let active = in_game && focused && has_window;
            let first = app.world_mut().resource_mut::<UiEffectQueue>().drain();
            assert_eq!(first.len(), if active { 2 } else { 0 });
            app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::InGame;
            let world = app.world_mut();
            if has_window {
                world
                    .query::<&mut Window>()
                    .single_mut(world)
                    .unwrap()
                    .focused = true;
            } else {
                world.spawn(Window {
                    focused: true,
                    ..default()
                });
            }
            app.update();
            assert_eq!(
                app.world_mut()
                    .resource_mut::<UiEffectQueue>()
                    .drain()
                    .len(),
                1,
                "only the new frame's intent remains after resume"
            );
        }
    }

    #[test]
    fn guild_notice_keeps_all_eight_text_lines_above_ime() {
        assert_eq!(player_editor_bottom(Some("guild-notice"), 1.0), 333.0);
    }

    #[test]
    fn every_shared_amount_dialog_uses_its_modal_geometry_for_ime() {
        for field in ["inventory-amount", "guild-amount", "trade-amount"] {
            assert_eq!(player_editor_bottom(Some(field), 1.0), 446.0);
        }
        assert_eq!(player_editor_bottom(Some("chat"), 1.0), 750.0);
    }

    #[test]
    fn mail_ime_footer_reflows_and_restores_without_replacing_shared_buttons() {
        use mir2_client_bevy::crystal_ui::overlays::{MailComposeDetails, MailComposeFooter};
        let mut app = App::new();
        app.init_resource::<HostState>()
            .init_resource::<UiScale>()
            .add_systems(Update, fit_mail_composer);
        let details = app
            .world_mut()
            .spawn((MailComposeDetails, Node::default()))
            .id();
        let button = app
            .world_mut()
            .spawn((
                Button,
                Node {
                    top: px(408),
                    height: px(28),
                    ..default()
                },
            ))
            .id();
        let footer = app
            .world_mut()
            .spawn((MailComposeFooter, Node::default()))
            .add_child(button)
            .id();
        // Disabled Send has no Button component, but must retain the same size.
        let disabled = app
            .world_mut()
            .spawn(Node {
                top: px(408),
                height: px(28),
                ..default()
            })
            .id();
        app.world_mut().entity_mut(footer).add_child(disabled);
        for scale in [0.35, 0.535, 1.0, 2.0] {
            app.world_mut().resource_mut::<UiScale>().0 = scale;
            for editing in [true, false, true, false] {
                app.world_mut().resource_mut::<HostState>().ime_bottom =
                    if editing { 500.0 } else { 0.0 };
                app.update();
                let footer_node = app.world().get::<Node>(footer).unwrap();
                let button_node = app.world().get::<Node>(button).unwrap();
                assert_eq!(
                    app.world().get::<Node>(details).unwrap().display,
                    if editing {
                        Display::None
                    } else {
                        Display::Flex
                    }
                );
                assert_eq!(footer_node.top, px(if editing { -298.0 } else { 0.0 }));
                let height = if editing {
                    (44.0 / scale).max(28.0)
                } else {
                    28.0
                };
                assert_eq!(button_node.height, px(height));
                assert_eq!(
                    app.world().get::<Node>(disabled).unwrap().height,
                    px(height)
                );
                assert_eq!(button_node.top, px(408));
                if editing {
                    assert!(height * scale >= 44.0);
                    let bottom = player_editor_bottom(Some("mail-message"), scale);
                    assert!(((bottom - 115.0 - height) * scale - 8.0).abs() < 0.001);
                    assert_eq!(bottom, player_editor_bottom(Some("mail-recipient"), scale));
                }
            }
        }
    }

    #[test]
    fn android_back_is_one_shared_escape_press_release_pair() {
        let mut events = Messages::default();
        let window = Entity::PLACEHOLDER;
        shared_back_key(&mut events, window);
        let sent: Vec<_> = events.drain().collect();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].state, bevy::input::ButtonState::Pressed);
        assert_eq!(sent[1].state, bevy::input::ButtonState::Released);
        assert!(sent.iter().all(|event| event.key_code == KeyCode::Escape
            && event.text.is_none()
            && !event.repeat));
    }

    #[test]
    fn repeated_editor_tap_reopens_but_held_press_does_not_reset_ime() {
        let mut app = App::new();
        app.init_resource::<EditorTouch>()
            .add_systems(Update, remember_editor_touch);
        let field = app
            .world_mut()
            .spawn((
                Interaction::Pressed,
                mir2_client_bevy::crystal_ui::overlays::NativeTextInputTarget,
            ))
            .id();
        app.update();
        assert!(app.world().resource::<EditorTouch>().0);
        app.update();
        assert!(!app.world().resource::<EditorTouch>().0);
        *app.world_mut().get_mut::<Interaction>(field).unwrap() = Interaction::None;
        app.update();
        *app.world_mut().get_mut::<Interaction>(field).unwrap() = Interaction::Pressed;
        app.update();
        assert!(app.world().resource::<EditorTouch>().0);
    }

    #[test]
    fn host_drives_shared_shell_without_a_parallel_form_or_fake_world() {
        INBOX.lock().unwrap().clear();
        OUTBOX.lock().unwrap().clear();
        let mut app = App::new();
        #[cfg(feature = "ui-preview")]
        app.init_resource::<crate::ui_preview::PreviewRequest>();
        app.init_resource::<NativeShellModel>()
            .init_resource::<HostState>()
            .init_resource::<NativeUiIntentQueue>()
            .add_systems(Update, (receive, forward_intents).chain());
        INBOX.lock().unwrap().push_back(json!({"phase":"READY"}));
        app.update();
        let mut model = app.world_mut().resource_mut::<NativeShellModel>();
        assert_eq!(model.screen, Screen::Login);
        model.login.account = "ui-fixture".into();
        model.login.password = "not-a-real-password".into();
        assert!(model.apply_ui_intent(Intent::Login));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(Intent::Login);
        app.update();
        assert!(app
            .world()
            .resource::<NativeShellModel>()
            .login
            .password
            .is_empty());
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(command["type"], "login");
        INBOX
            .lock()
            .unwrap()
            .push_back(json!({"phase":"CHARACTERS","characters":[
            {"index":7,"name":"Fixture","level":12,"className":"Wizard","genderName":"Female"}]}));
        app.update();
        let mut model = app.world_mut().resource_mut::<NativeShellModel>();
        assert_eq!(model.screen, Screen::OpeningLogin);
        model.advance_login_opening(std::time::Duration::from_millis(1800));
        assert_eq!(model.screen, Screen::CharacterSelect);
        assert_eq!(model.characters[0].class_name, "Wizard");
        assert_eq!(model.selected_character_index, Some(7));
        assert!(model.apply_ui_intent(Intent::StartGame));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(Intent::StartGame);
        app.update();
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(command["index"], 7);
        INBOX.lock().unwrap().push_back(
            json!({"phase":"IN_GAME","message":"Server position available; scene pending"}),
        );
        app.update();
        assert_ne!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::InGame
        );
        let character = app.world().resource::<NativeShellModel>().characters[0].clone();
        assert!(app
            .world_mut()
            .resource_mut::<NativeShellModel>()
            .apply_gateway_event(Event::PlayerBootstrapped {
                character: character.clone(),
            }));
        {
            let mut host = app.world_mut().resource_mut::<HostState>();
            host.phase = "IN_GAME".into();
            host.world = Some(HostWorldPosition {
                player_name: character.name.clone(),
                map_file_name: "0".into(),
                x: 300,
                y: 630,
            });
        }
        INBOX
            .lock()
            .unwrap()
            .push_back(json!({"phase":"STARTING","message":"Waiting for destination snapshot"}));
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::StartingGame
        );
        assert_eq!(
            app.world()
                .resource::<NativeShellModel>()
                .active_character
                .as_ref(),
            Some(&character)
        );
        let host = app.world().resource::<HostState>();
        assert_eq!(host.phase, "STARTING");
        assert!(host.world.is_none());
        assert!(host.pending_world_request.is_none());
        assert!(host.pending_render_request.is_none());
        INBOX
            .lock()
            .unwrap()
            .push_back(json!({"phase":"DISCONNECTED","message":"Backgrounded"}));
        app.update();
        let model = app.world().resource::<NativeShellModel>();
        assert_eq!(model.screen, Screen::ConnectionLost);
        assert!(model.characters.is_empty());
        assert!(OUTBOX.lock().unwrap().is_empty());
    }
}
