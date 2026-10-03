//! Android adapter for the SAME Crystal shell used by the Windows host.
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
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
static PRESENTATION_OUTBOX: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
const MAX_PRESENTATION_EVENTS: usize = 64;

pub(crate) fn enqueue_presentation_event(event: String) {
    let mut queue = PRESENTATION_OUTBOX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if queue.len() >= MAX_PRESENTATION_EVENTS {
        queue.clear();
        queue.push_back(json!({"type":"clear", "atMs":0}).to_string());
    }
    queue.push_back(event);
}

#[derive(Default)]
pub(crate) struct AndroidPresentationMainThread;

fn flush_presentation_events(_main_thread: NonSend<AndroidPresentationMainThread>) {
    // The shared runtime's movement bridge is thread-local for the WASM host.
    // Drain Android's synchronized host queue from a NonSend system so both
    // enablement and packets reach the renderer's main-thread consumer.
    mir2_bevy_runtime::set_mir2_remote_motion_presentation_enabled(true);
    let events = PRESENTATION_OUTBOX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .drain(..)
        .collect::<Vec<_>>();
    for event in events {
        mir2_bevy_runtime::push_mir2_movement_shadow_event(event);
    }
}

fn enqueue_host_event(queue: &mut VecDeque<Value>, text: &str) {
    // Snapshot JSON is escaped once inside the host envelope. Retain the small
    // limit for ordinary input events and bound aggregate snapshot memory too.
    let parsed = (text.len() <= 2 * 1024 * 1024 + 65536)
        .then(|| serde_json::from_str::<Value>(text).ok())
        .flatten();
    let valid = parsed.as_ref().is_some_and(|value| {
        let snapshot_bytes = value["worldSnapshot"].as_str().map_or(0, str::len);
        let personal_bytes = large_personal_host_payload_bytes(value);
        snapshot_bytes <= 1024 * 1024
            && text.len() <= 65536 + 2 * snapshot_bytes + 2 * personal_bytes
            && queue.len() < 32
            && queue
                .iter()
                .map(|event| {
                    65536
                        + event["worldSnapshot"].as_str().map_or(0, str::len)
                        + event["envelope"].as_str().map_or(0, str::len)
                })
                .sum::<usize>()
                + snapshot_bytes
                + value["envelope"].as_str().map_or(0, str::len)
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

/// A complete public mailbox gets its own hard byte cap. This only affects
/// host-envelope residency; the authenticated phase/owner/model gates below
/// still decide admission. Other event types keep their existing small cap.
fn large_mail_host_payload_bytes(value: &Value) -> usize {
    if value["type"] != "gatewayGameplayPacket" {
        return 0;
    }
    let Some(raw) = value["envelope"]
        .as_str()
        .filter(|raw| raw.len() <= crate::mail_ingress::MAX_MAIL_PACKET_BYTES)
    else {
        return 0;
    };
    if serde_json::from_str::<Value>(raw)
        .ok()
        .is_some_and(|event| event["type"] == "packet" && event["packet"] == "ReceiveMail")
    {
        raw.len()
    } else {
        0
    }
}

fn large_personal_host_payload_bytes(value: &Value) -> usize {
    let mail_bytes = large_mail_host_payload_bytes(value);
    if mail_bytes != 0 {
        return mail_bytes;
    }
    if value["type"] != "gatewayGameplayPacket" {
        return 0;
    }
    let Some(raw) = value["envelope"].as_str()
        .filter(|raw| raw.len() <= crate::social_ingress::MAX_SOCIAL_PACKET_BYTES)
    else { return 0; };
    if serde_json::from_str::<Value>(raw).ok().is_some_and(|event|
        event["type"] == "packet" && event["packet"].as_str()
            .is_some_and(crate::social_ingress::is_large_social_packet_name))
    {
        raw.len()
    } else {
        0
    }
}

fn scene_render_packet(raw: &str) -> Option<&str> {
    // Personal mail/social already passed dedicated phase/owner decoders.
    // A host-only receipt is not a public scene envelope, and a full mailbox
    // exceeds the scene decoders' small caps. Neither can affect world objects.
    // Unknown or malformed other input retains fail-closed scene validation.
    // Classification does not authorize personal-data admission.
    (!crate::mail_ingress::is_mail_packet(raw)
        && !crate::social_ingress::is_social_packet(raw)).then_some(raw)
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
    chat: crate::chat_ingress::AndroidChatIngress,
    skills: crate::skill_ingress::AndroidSkillIngress,
    inventory: crate::inventory_ingress::AndroidInventoryIngress,
    game_shop: crate::game_shop_ingress::AndroidGameShopIngress,
    storage: crate::storage_ingress::AndroidStorageIngress,
    mail: crate::mail_ingress::AndroidMailIngress,
    social: crate::social_ingress::AndroidSocialIngress,
    npc: crate::npc_ingress::AndroidNpcIngress,
    player: crate::player_ingress::AndroidPlayerIngress,
    pub(crate) quests: crate::quest_ingress::AndroidQuestIngress,
    pub(crate) phase: String,
    world: Option<HostWorldPosition>,
    pending_world_request: Option<u64>,
    pending_render_request: Option<u64>,
    pub(crate) ime_bottom: f32,
    pub(crate) safe_right: f32,
    pub(crate) safe_top: f32,
    safe_left: f32,
    safe_bottom: f32,
    render_load_active: bool,
    deferred_render_load: Option<DeferredRenderLoad>,
    editor: HostEditorSession,
}

impl HostState {
    /// Physical Android presentation insets, including the active IME. Keep
    /// host fields private; the UI caller converts density exactly once.
    pub(crate) fn quest_presentation_insets(&self) -> Vec4 {
        Vec4::new(
            self.safe_left,
            self.safe_top,
            self.safe_right,
            self.safe_bottom + self.ime_bottom,
        )
    }

    fn reset_personal(&mut self) {
        self.chat.reset();
        self.skills.reset();
        self.inventory.reset();
        self.game_shop.reset();
        self.storage.reset();
        self.mail.reset();
        self.social.reset();
        self.npc.reset();
        self.player.reset();
        self.quests.reset();
    }

    fn reset_gameplay(&mut self) {
        self.reset_personal();
        self.phase = "DISCONNECTED".into();
        self.world = None;
        self.pending_world_request = None;
        self.pending_render_request = None;
        self.render_load_active = false;
        self.deferred_render_load = None;
    }

    fn flush_personal(&mut self) {
        self.chat
            .flush(mir2_bevy_runtime::native_ingest::push_native_chat_line);
        self.skills
            .flush(mir2_bevy_runtime::native_ingest::push_native_skill_model);
        self.inventory.flush(
            mir2_bevy_runtime::native_ingest::push_native_inventory_model,
            mir2_bevy_runtime::native_ingest::push_native_inventory_operation_ack,
        );
        self.player.flush(
            mir2_bevy_runtime::native_ingest::push_native_ui_read_model,
            mir2_bevy_runtime::native_ingest::push_native_wallet_patch,
        );
        self.game_shop.flush(
            mir2_bevy_runtime::native_ingest::push_native_game_shop_info,
            mir2_bevy_runtime::native_ingest::push_native_game_shop_stock,
        );
        self.storage.flush(
            mir2_bevy_runtime::native_ingest::push_native_storage_model,
            mir2_bevy_runtime::native_ingest::push_native_storage_items,
            mir2_bevy_runtime::native_ingest::push_native_storage_patch,
        );
        self.mail.flush(
            mir2_bevy_runtime::native_ingest::push_native_mail_model,
            mir2_bevy_runtime::native_ingest::push_native_mail_service,
        );
        self.social.flush(mir2_bevy_runtime::native_ingest::push_native_social_model);
    }

    fn accept_player_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.player.packet(raw)
    }

    fn accept_npc_packet(
        &mut self,
        screen: Screen,
        raw: &str,
        accepts: bool,
    ) -> Result<bool, &'static str> {
        if self.phase != "IN_GAME" || screen != Screen::InGame {
            return Ok(false);
        }
        self.npc
            .packet(raw, accepts, self.player.presentation_cursor())
    }

    fn flush_npc(&mut self, screen: Screen, accepts: bool) {
        self.npc.flush(
            self.phase == "IN_GAME" && screen == Screen::InGame && accepts,
            mir2_bevy_runtime::native_ingest::push_native_shop_model,
            mir2_bevy_runtime::native_ingest::push_native_npc_shop_service,
        );
    }

    fn bind_npc_snapshot(&mut self, raw: &str) -> Result<(), &'static str> {
        self.npc.snapshot(raw, self.player.presentation_cursor())
    }

    fn bind_chat_owner(&mut self) -> Result<(), &'static str> {
        let (owner, name) = self
            .player
            .identity()
            .ok_or("Missing chat owner bootstrap")?;
        self.chat.bind(owner, name)
    }

    fn accept_chat_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.chat.packet(raw)
    }

    fn accept_quest_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.quests.packet(raw)
    }

    fn bind_quest_snapshot(&mut self, raw: &str) -> Result<(), &'static str> {
        self.quests.snapshot(raw, self.player.presentation_cursor())
    }

    fn accept_game_shop_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.game_shop
            .packet(raw, self.player.presentation_cursor())
    }

    fn bind_game_shop_snapshot(&mut self, raw: &str) -> Result<(), &'static str> {
        self.game_shop
            .snapshot(raw, self.player.presentation_cursor())
    }

    fn accept_inventory_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.inventory.packet(raw)
    }

    fn accept_storage_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.storage.packet(raw)
    }

    fn accept_mail_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.mail.packet(raw)
    }

    fn accept_skill_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.skills.packet(raw)
    }

    fn accept_social_packet(&mut self, screen: Screen, raw: &str) -> Result<bool, &'static str> {
        if !matches!(self.phase.as_str(), "STARTING" | "IN_GAME")
            || !matches!(screen, Screen::StartingGame | Screen::InGame)
        {
            return Ok(false);
        }
        self.social.packet(raw, self.player.presentation_cursor())
    }

    fn bind_social_owner(&mut self) -> Result<(), &'static str> {
        let (owner, name) = self.player.identity().ok_or("Missing social owner")?;
        self.social.bind(owner, name, self.player.presentation_cursor())
    }
}

/// OS callbacks belong to one activation, not merely a reusable field name.
#[derive(Default)]
struct HostEditorSession {
    epoch: u64,
    field: Option<String>,
}

impl HostEditorSession {
    fn open(&mut self, field: &str) -> u64 {
        self.epoch = self.epoch.checked_add(1).unwrap_or(1);
        self.field = Some(field.to_owned());
        self.epoch
    }

    fn close(&mut self) {
        self.field = None;
    }

    fn accepts(&self, event: &Value) -> bool {
        self.field.is_some()
            && event["editorEpoch"].as_u64() == Some(self.epoch)
            && event["field"].as_str() == self.field.as_deref()
    }
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

fn host_account_event(value: &Value) -> Option<Event> {
    let event = value.get("accountEvent")?.as_object()?;
    match event.get("type")?.as_str()? {
        "characterCreated" => {
            let character = event.get("character")?.as_object()?;
            let index = i32::try_from(character.get("index")?.as_i64()?).ok()?;
            let name = character.get("name")?.as_str()?;
            let class_name = character.get("className")?.as_str()?;
            let gender_name = character.get("genderName")?.as_str()?;
            if index < 0
                || [name, class_name, gender_name]
                    .iter()
                    .any(|text| text.trim().is_empty() || text.chars().count() > 128)
            {
                return None;
            }
            Some(Event::CharacterCreated {
                character: CharacterSummary::new(
                    index,
                    name,
                    character
                        .get("level")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .min(u16::MAX as u64) as u16,
                    class_name,
                    gender_name,
                ),
            })
        }
        "characterDeleted" => Some(Event::CharacterDeleted {
            character_index: i32::try_from(event.get("characterIndex")?.as_i64()?).ok()?,
        }),
        "operationFailure" => {
            let message = event.get("message")?.as_str()?;
            (message.chars().count() <= 256).then(|| Event::OperationFailure {
                message: message.to_owned(),
            })
        }
        _ => None,
    }
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

fn fail_current_render_load(
    host: &mut HostState,
    model: &mut NativeShellModel,
    request_id: u64,
    message: &str,
) -> bool {
    if host.pending_render_request != Some(request_id) {
        return false;
    }
    host.reset_gameplay();
    model.apply_gateway_event(Event::Disconnect {
        reason: Some(format!(
            "World assets could not be loaded: {message}; reconnect"
        )),
    });
    true
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

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum AndroidShellBleedEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Component)]
struct AndroidShellBleed(AndroidShellBleedEdge);

const SHELL_STAGE_WIDTH: f32 = 1024.0;
const SHELL_STAGE_HEIGHT: f32 = 768.0;
const SHELL_SIDE_BLEED_SOURCE_WIDTH: f32 = 160.0;
const SHELL_VERTICAL_BLEED_SOURCE_HEIGHT: f32 = 96.0;

fn spawn_android_shell_bleed(mut commands: Commands, asset_server: Res<AssetServer>) {
    let background = asset_server.load("original-ui/ChrSel/0.png");
    for edge in [
        AndroidShellBleedEdge::Left,
        AndroidShellBleedEdge::Right,
        AndroidShellBleedEdge::Top,
        AndroidShellBleedEdge::Bottom,
    ] {
        let (rect, flip_x, flip_y) = match edge {
            AndroidShellBleedEdge::Left => (
                Rect::new(0.0, 0.0, SHELL_SIDE_BLEED_SOURCE_WIDTH, SHELL_STAGE_HEIGHT),
                true,
                false,
            ),
            AndroidShellBleedEdge::Right => (
                Rect::new(
                    SHELL_STAGE_WIDTH - SHELL_SIDE_BLEED_SOURCE_WIDTH,
                    0.0,
                    SHELL_STAGE_WIDTH,
                    SHELL_STAGE_HEIGHT,
                ),
                true,
                false,
            ),
            AndroidShellBleedEdge::Top => (
                Rect::new(
                    0.0,
                    0.0,
                    SHELL_STAGE_WIDTH,
                    SHELL_VERTICAL_BLEED_SOURCE_HEIGHT,
                ),
                false,
                true,
            ),
            AndroidShellBleedEdge::Bottom => (
                Rect::new(
                    0.0,
                    SHELL_STAGE_HEIGHT - SHELL_VERTICAL_BLEED_SOURCE_HEIGHT,
                    SHELL_STAGE_WIDTH,
                    SHELL_STAGE_HEIGHT,
                ),
                false,
                true,
            ),
        };
        commands.spawn((
            AndroidShellBleed(edge),
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                ..default()
            },
            ImageNode {
                image: background.clone(),
                rect: Some(rect),
                flip_x,
                flip_y,
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
            FocusPolicy::Pass,
            GlobalZIndex(999),
        ));
    }
}

fn shell_bleed_node(
    edge: AndroidShellBleedEdge,
    fit: CrystalStageTransform,
    shell_visible: bool,
) -> Node {
    let logical_width = fit.viewport_width / fit.scale;
    let logical_height = fit.viewport_height / fit.scale;
    let side_gutter = fit.offset_x / fit.scale;
    let vertical_gutter = fit.offset_y / fit.scale;
    let (left, top, width, height) = match edge {
        AndroidShellBleedEdge::Left => (0.0, 0.0, side_gutter, logical_height),
        AndroidShellBleedEdge::Right => (
            side_gutter + SHELL_STAGE_WIDTH,
            0.0,
            side_gutter,
            logical_height,
        ),
        AndroidShellBleedEdge::Top => (0.0, 0.0, logical_width, vertical_gutter),
        AndroidShellBleedEdge::Bottom => (
            0.0,
            vertical_gutter + SHELL_STAGE_HEIGHT,
            logical_width,
            vertical_gutter,
        ),
    };
    Node {
        position_type: PositionType::Absolute,
        left: px(left),
        top: px(top),
        width: px(width),
        height: px(height),
        display: if shell_visible && width > 0.01 && height > 0.01 {
            Display::Flex
        } else {
            Display::None
        },
        ..default()
    }
}

fn fit_shell_bleed(
    windows: Query<&Window>,
    model: Res<NativeShellModel>,
    mut shell_bleed: Query<(&AndroidShellBleed, &mut Node)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let fit = CrystalStageTransform::fit(window.width(), window.height());
    for (edge, mut node) in &mut shell_bleed {
        *node = shell_bleed_node(edge.0, fit, model.screen != Screen::InGame);
    }
}

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
            .insert_non_send(AndroidPresentationMainThread)
            .init_resource::<HostState>()
            .init_resource::<EditorTouch>()
            .add_plugins(Mir2NativeShellUiPlugin)
            .add_systems(Startup, spawn_android_shell_bleed)
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
            .add_systems(PreUpdate, flush_presentation_events.after(receive))
            .add_systems(
                PreUpdate,
                remember_editor_touch.after(bevy::ui::UiSystems::Focus),
            )
            .add_systems(
                PostUpdate,
                (
                    observe_world_receipt,
                    observe_render_receipt,
                    fit_shell_bleed,
                    fit_stage.in_set(AndroidStageFit),
                    forward_intents,
                    forward_quest_ui_intents,
                    forward_native_mail_ui_intents,
                    discard_inactive_player_commands,
                    keep_android_ime_owned_by_host,
                    keyboard,
                )
                    .chain()
                    .before(bevy::ui::UiSystems::Layout),
            )
            .add_systems(
                PostUpdate,
                crate::android_ui_clipping::update_android_clipping
                    .after(bevy::ui::UiSystems::PostLayout),
            );
        #[cfg(feature = "ui-preview")]
        crate::ui_preview::install(app);
        #[cfg(feature = "ui-preview")]
        app.add_systems(
            PostUpdate,
            (report_preview_social_layout, report_preview_mail_layout)
                .after(bevy::ui::UiSystems::Layout),
        );
        crate::quest_ingress::install(app);
        crate::entity_overlays::install(app);
        crate::ground_labels::install(app);
        crate::mobile_ui::install(app);
        crate::phone_hud::install(app);
        crate::phone_quests::install(app);
        app.init_resource::<mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::phone::PhoneGameShopPresentation>()
            .add_systems(Update, publish_game_shop_phone
                .before(mir2_client_bevy::crystal_ui::overlays::NativePlayerUiSet::Mutate));
        #[cfg(feature = "ui-preview")]
        app.add_systems(PostUpdate, report_game_shop_phone
            .after(bevy::ui::UiSystems::PostLayout));
        crate::scene_effects::install(app);
    }
}

fn publish_game_shop_phone(
    windows: Query<&Window>,
    host: Res<HostState>,
    mut presentation: ResMut<mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::phone::PhoneGameShopPresentation>,
) {
    use mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::phone::PhoneGameShopPresentation;
    let next = windows.single().ok().map(|window| {
        let viewport = Vec2::new(window.width(), window.height());
        let fit = CrystalStageTransform::fit(viewport.x, viewport.y);
        let safe = Vec4::new(host.safe_left, host.safe_top, host.safe_right,
            host.safe_bottom + host.ime_bottom) / window.scale_factor();
        PhoneGameShopPresentation::fit(viewport, safe, fit.scale)
    }).filter(|p| p.is_valid()).unwrap_or_default();
    if *presentation != next { *presentation = next; }
}

#[cfg(feature = "ui-preview")]
fn report_game_shop_phone(
    windows: Query<&Window>,
    stage: Res<UiScale>,
    player: Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    texts: Query<(&Text, &TextFont, &bevy::text::TextLayoutInfo),
        With<mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::phone::PhoneGameShopText>>,
    controls: Query<(&ComputedNode, &UiGlobalTransform,
        &mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::phone::PhoneGameShopControl)>,
    areas: Query<&ComputedNode,
        With<mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::phone::PhoneGameShopScrollArea>>,
    mut frames: Local<u16>,
) {
    *frames = frames.wrapping_add(1);
    if *frames % 60 != 0 || !player.shop_open() { return; }
    let Ok(window) = windows.single() else { return };
    let density = window.scale_factor();
    for (text, font, layout) in &texts {
        if let FontSize::Px(size) = font.font_size {
            info!(text=?text.0, font_dp=size*stage.0, glyphs=layout.glyphs.len(),
                alpha_masks=layout.glyphs.iter().filter(|g| g.atlas_info.is_alpha_mask).count(),
                "ANDROID_PHONE_GAMESHOP_TEXT");
        }
    }
    for (node, transform, control) in &controls {
        info!(control=?control, size_dp=?node.size()/density,
            center_dp=?transform.translation/density, "ANDROID_PHONE_GAMESHOP_CONTROL");
    }
    for node in &areas {
        info!(size_dp=?node.size()/density, content_dp=?node.content_size()/density,
            offset=?node.scroll_position*node.inverse_scale_factor, "ANDROID_PHONE_GAMESHOP_SCROLL");
    }
}

#[cfg(feature = "ui-preview")]
fn report_preview_mail_layout(
    host: Res<HostState>,
    player: Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    windows: Query<&Window>,
    roots: Query<
        (&Node, &ComputedNode, &UiTransform, &UiGlobalTransform),
        With<mir2_client_bevy::crystal_ui::overlays::OverlayMailComposer>,
    >,
    mut previous: Local<String>,
) {
    if !player.mail_open() {
        return;
    }
    let layout = format!(
        "compose={} ime={} editor={} window={:?} roots={:?}",
        player.core.mail_compose.is_some(),
        host.ime_bottom,
        host.editor.field.is_some(),
        windows
            .single()
            .ok()
            .map(|window| (window.width(), window.height(), window.focused)),
        roots
            .iter()
            .map(|(node, computed, transform, global)| {
                (
                    node.display,
                    node.left,
                    node.top,
                    node.width,
                    node.height,
                    computed.size(),
                    transform,
                    global,
                )
            })
            .collect::<Vec<_>>()
    );
    if *previous != layout {
        info!("ANDROID_MAIL_LAYOUT {layout}");
        *previous = layout;
    }
}

#[cfg(feature = "ui-preview")]
fn report_preview_social_layout(
    roots: Query<
        (&UiTransform, &UiGlobalTransform, &Children),
        Or<(
            With<mir2_client_bevy::crystal_ui::overlays::OverlaySocialFocus>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayTrade>,
        )>,
    >,
    globals: Query<&UiGlobalTransform>,
    mut logged: Local<bool>,
) {
    if *logged {
        return;
    }
    for (transform, global, children) in &roots {
        if *transform == UiTransform::default() || children.is_empty() {
            continue;
        }
        info!(
            ?transform,
            ?global,
            first_child_global = ?globals.get(children[0]).ok(),
            "ANDROID_SOCIAL_LAYOUT_COMPUTED"
        );
        *logged = true;
        break;
    }
}

#[cfg(test)]
#[path = "phone_panel_tests.rs"]
mod phone_panel_tests;

fn fit_stage(
    windows: Query<&Window>,
    phone_shop: Option<Res<mir2_client_bevy::crystal_ui::overlays::game_shop_dialog::phone::PhoneGameShopPresentation>>,
    host: Res<HostState>,
    model: Res<NativeShellModel>,
    player: Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
    npc_dialog: Res<mir2_client_bevy::quest_model::NpcDialogModel>,
    forms: crate::form_input::FormInput,
    mut scale: ResMut<UiScale>,
    belt: Res<mir2_client_bevy::crystal_ui::hud::CrystalBeltPresentation>,
    #[cfg(feature = "ui-preview")] mut preview_focus_logged: Local<u8>,
    mut roots: Query<
        (
            &mut Node,
            Has<mir2_client_bevy::crystal_ui::minimap::CrystalMiniMapRoot>,
            Has<mir2_client_bevy::crystal_ui::hud::CrystalHudRoot>,
            Has<mir2_client_bevy::crystal_ui::chat::CrystalChatRoot>,
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
            Without<mir2_client_bevy::crystal_ui::chat::CrystalChatSettingsModal>,
            (
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayInventory>,
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayEquipment>,
            ),
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayStorage>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayOptions>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayShop>,
            (
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayMail>,
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayMailWindow>,
            ),
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayGameShop>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayBigMap>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlaySocial>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlaySocialFocus>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayTrade>,
            Without<mir2_client_bevy::quest_ui::NpcDialogPanel>,
        ),
    >,
    mut minimap_layers: Query<
        &mut Node,
        (
            With<mir2_client_bevy::crystal_ui::hud::CrystalHudMiniMapLayer>,
            Without<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>,
            Without<mir2_client_bevy::crystal_ui::chat::CrystalChatSettingsModal>,
            (
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayInventory>,
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayEquipment>,
            ),
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayStorage>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayOptions>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayShop>,
            (
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayMail>,
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayMailWindow>,
            ),
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayGameShop>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayBigMap>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlaySocial>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlaySocialFocus>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayTrade>,
            Without<mir2_client_bevy::quest_ui::NpcDialogPanel>,
        ),
    >,
    mut belt_layers: Query<
        &mut Node,
        (
            With<mir2_client_bevy::crystal_ui::hud::CrystalHudBeltLayer>,
            Without<mir2_client_bevy::crystal_ui::chat::CrystalChatSettingsModal>,
            (
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayInventory>,
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayEquipment>,
            ),
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayStorage>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayOptions>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayShop>,
            (
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayMail>,
                Without<mir2_client_bevy::crystal_ui::overlays::OverlayMailWindow>,
            ),
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayGameShop>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayBigMap>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlaySocial>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlaySocialFocus>,
            Without<mir2_client_bevy::crystal_ui::overlays::OverlayTrade>,
            Without<mir2_client_bevy::quest_ui::NpcDialogPanel>,
        ),
    >,
    mut focus_panels: Query<
        (
            &Node,
            &mut UiTransform,
            Has<mir2_client_bevy::crystal_ui::chat::CrystalChatSettingsModal>,
            (
                Has<mir2_client_bevy::crystal_ui::overlays::OverlayInventory>,
                Has<mir2_client_bevy::crystal_ui::overlays::OverlayEquipment>,
            ),
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayStorage>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayOptions>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayShop>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayMail>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayMailWindow>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayMailComposer>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayGameShop>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayBigMap>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlaySocialFocus>,
            Has<mir2_client_bevy::crystal_ui::overlays::OverlayTrade>,
            Has<mir2_client_bevy::quest_ui::NpcDialogPanel>,
        ),
        Or<(
            With<mir2_client_bevy::crystal_ui::chat::CrystalChatSettingsModal>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayInventory>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayEquipment>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayStorage>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayOptions>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayShop>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayMail>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayMailWindow>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayGameShop>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayBigMap>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlaySocialFocus>,
            With<mir2_client_bevy::crystal_ui::overlays::OverlayTrade>,
            With<mir2_client_bevy::quest_ui::NpcDialogPanel>,
        )>,
    >,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let fit = CrystalStageTransform::fit(window.width(), window.height());
    let focused_field = forms.field(&player).map(|field| field.0);
    // Keep field/button size stable while typing. Pan the shared stage to keep
    // its login panel above the keyboard instead of shrinking it to a thumbnail.
    let available = (window.height() - host.ime_bottom / window.scale_factor()).max(1.0);
    let panel = mir2_client_bevy::crystal_ui::spec::login::PANEL.rect;
    let top = if host.ime_bottom > 0.0 && model.screen == Screen::InGame {
        let field =
            focused_field.or_else(|| crate::text_input::player_field(&player).map(|field| field.0));
        let editor_bottom = player_editor_bottom(field, fit.scale);
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
    let occlude_world_chrome = player.shop_open()
        || player.bigmap_open()
        || player.group_open()
        || player.guild_open()
        || player.trade_open();
    for (mut root, is_map_image, is_hud, is_chat) in &mut roots {
        root.left = px(if is_map_image {
            map_origin.x
        } else {
            fit.offset_x / fit.scale
        });
        root.top = px(if is_map_image { map_origin.y } else { top });
        if is_hud {
            root.display = if model.screen == Screen::InGame && !occlude_world_chrome {
                Display::Flex
            } else {
                Display::None
            };
        } else if is_chat {
            // GameShop and BigMap become near-full-height blocking panels on a
            // phone. Do not leave the desktop chat canvas visible or pickable
            // through their transparent Crystal artwork; closing the panel
            // restores the same shared chat tree on the next frame.
            root.display = if occlude_world_chrome {
                Display::None
            } else {
                Display::Flex
            };
        }
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
    let safe_edges = Vec4::new(
        host.safe_left / window.scale_factor(),
        host.safe_top / window.scale_factor(),
        host.safe_right / window.scale_factor(),
        (host.safe_bottom + host.ime_bottom) / window.scale_factor(),
    );
    let sidebar = crate::phone_panels::panel_sidebar(
        Vec2::new(window.width(), window.height()),
        Vec4::new(
            host.safe_left,
            host.safe_top,
            host.safe_right,
            host.safe_bottom,
        ) / window.scale_factor(),
        host.ime_bottom / window.scale_factor(),
        crate::phone_panels::sidebar_requested(&player),
    );
    for (
        node,
        mut transform,
        is_chat_settings,
        (is_inventory, is_equipment),
        is_storage,
        is_options,
        is_npc_shop,
        is_mail,
        is_mail_window,
        is_mail_composer,
        is_game_shop,
        is_bigmap,
        is_social,
        is_trade,
        is_npc_dialog,
    ) in &mut focus_panels
    {
        let geometry = focus_panel_rect(node);
        let Some((origin, size)) = geometry else {
            *transform = UiTransform::default();
            continue;
        };
        let enabled = is_chat_settings
            || (is_inventory && player.inventory_open() && !player.trade_dialog.open)
            || (is_equipment && player.equipment_open() && !player.storage_open())
            || (is_storage && player.storage_open())
            || (is_options && player.options_open())
            || (is_npc_shop && player.npc_shop_open())
            || (is_mail && player.mail_open())
            || (is_mail_window && node.display != Display::None)
            || (is_game_shop && player.shop_open())
            || (is_bigmap && player.bigmap_open())
            || (is_social && (player.group_open() || player.guild_open()))
            || (is_trade && player.trade_dialog.open)
            || (is_npc_dialog && npc_dialog.is_open);
        *transform = if enabled {
            let max_scale =
                if is_npc_shop || is_game_shop || is_bigmap || (is_social && player.guild_open()) {
                    1.8
                } else if is_npc_dialog || is_trade {
                    2.2
                } else {
                    3.2
                };
            let focused = if is_game_shop && phone_shop.as_deref().is_some_and(|phone| phone.is_valid()) {
                // An authored_unit phone panel already has dp size. Fit the
                // SAME safe/IME workspace, not the larger desktop focus scale.
                let workspace = Rect::from_corners(
                    Vec2::new(safe_edges.x + 16.0, safe_edges.y + 16.0),
                    Vec2::new(window.width() - safe_edges.z - 16.0, window.height() - safe_edges.w - 16.0),
                );
                mobile_workspace_transform(fit, origin, size, workspace, top, 1.0)
            } else if let Some(sidebar) = sidebar.filter(|_| is_inventory || is_equipment) {
                mobile_workspace_transform(fit, origin, size, sidebar.workspace, top, max_scale)
            } else {
                mobile_focus_transform(
                    fit,
                    origin,
                    size,
                    Vec2::new(window.width(), window.height()),
                    safe_edges,
                    top,
                    max_scale,
                )
            };
            if is_mail_composer && host.ime_bottom > 0.0 && focused_field == Some("mail-message") {
                let caret_bottom = forms.mail_body_top()
                    + forms
                        .mail_caret()
                        .map(|caret| caret.y)
                        .unwrap_or(165.0)
                        .clamp(0.0, 165.0)
                    + 4.0;
                keep_local_y_above_ime(
                    focused,
                    origin,
                    size,
                    caret_bottom,
                    fit,
                    Vec2::new(window.width(), window.height()),
                    safe_edges,
                    top,
                )
            } else if is_bigmap && host.ime_bottom > 0.0 && focused_field == Some("map-search") {
                keep_local_y_above_ime(
                    focused,
                    origin,
                    size,
                    488.0,
                    fit,
                    Vec2::new(window.width(), window.height()),
                    safe_edges,
                    top,
                )
            } else {
                focused
            }
        } else {
            UiTransform::default()
        };
        #[cfg(feature = "ui-preview")]
        if enabled && (is_social || is_trade) {
            let bit = if is_trade {
                4
            } else if player.guild_open() {
                2
            } else {
                1
            };
            if *preview_focus_logged & bit == 0 {
                info!(
                    group = player.group_open(),
                    guild = player.guild_open(),
                    trade = player.trade_dialog.open,
                    ?origin,
                    ?size,
                    transform = ?*transform,
                    "ANDROID_SOCIAL_FOCUS_APPLIED"
                );
                *preview_focus_logged |= bit;
            }
        }
    }
}

/// Grow compact shared dialogs inside the Android safe viewport while leaving
/// their authored 1024x768 coordinates intact. `UiTransform` is applied to the
/// real shared panel, so Bevy picking follows the exact same geometry.
fn mobile_focus_scale(
    fit: CrystalStageTransform,
    panel_size: Vec2,
    safe_width: f32,
    safe_height: f32,
    max_scale: f32,
) -> f32 {
    const EDGE_GUTTER: f32 = 16.0;
    let available_width = (safe_width - EDGE_GUTTER * 2.0).max(1.0);
    let available_height = (safe_height - EDGE_GUTTER * 2.0).max(1.0);
    let fitted_width = (panel_size.x * fit.scale).max(f32::EPSILON);
    let fitted_height = (panel_size.y * fit.scale).max(f32::EPSILON);
    (available_width / fitted_width)
        .min(available_height / fitted_height)
        .clamp(1.0, max_scale.max(1.0))
}

fn focus_panel_rect(node: &Node) -> Option<(Vec2, Vec2)> {
    let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
        (node.left, node.top, node.width, node.height)
    else {
        return None;
    };
    (width > 0.0 && height > 0.0).then_some((Vec2::new(left, top), Vec2::new(width, height)))
}

fn mobile_focus_transform(
    fit: CrystalStageTransform,
    origin: Vec2,
    size: Vec2,
    viewport: Vec2,
    safe_edges: Vec4,
    root_top: f32,
    max_scale: f32,
) -> UiTransform {
    const EDGE_GUTTER: f32 = 16.0;
    let safe_width = (viewport.x - safe_edges.x - safe_edges.z).max(1.0);
    let safe_height = (viewport.y - safe_edges.y - safe_edges.w).max(1.0);
    let scale = mobile_focus_scale(fit, size, safe_width, safe_height, max_scale);
    let min = Vec2::new(
        (safe_edges.x + EDGE_GUTTER - fit.offset_x) / fit.scale,
        (safe_edges.y + EDGE_GUTTER) / fit.scale - root_top,
    );
    let max = Vec2::new(
        (viewport.x - safe_edges.z - EDGE_GUTTER - fit.offset_x) / fit.scale,
        (viewport.y - safe_edges.w - EDGE_GUTTER) / fit.scale - root_top,
    );
    let half = size * scale * 0.5;
    let center = origin + size * 0.5;
    let clamp_center = |value: f32, lower: f32, upper: f32| {
        if lower <= upper {
            value.clamp(lower, upper)
        } else {
            (lower + upper) * 0.5
        }
    };
    let target = Vec2::new(
        clamp_center(center.x, min.x + half.x, max.x - half.x),
        clamp_center(center.y, min.y + half.y, max.y - half.y),
    );
    UiTransform {
        translation: Val2::px(target.x - center.x, target.y - center.y),
        scale: Vec2::splat(scale),
        ..default()
    }
}

/// Center a shared panel in the SAME logical-pixel workspace that the phone
/// HUD uses. Leave authored Node geometry intact for shared item/picking code.
fn mobile_workspace_transform(
    fit: CrystalStageTransform,
    origin: Vec2,
    size: Vec2,
    workspace: Rect,
    root_top: f32,
    max_scale: f32,
) -> UiTransform {
    let scale = (workspace.width() / (size.x * fit.scale))
        .min(workspace.height() / (size.y * fit.scale))
        .clamp(0.01, max_scale.max(0.01));
    let target = Vec2::new(
        (workspace.center().x - fit.offset_x) / fit.scale,
        workspace.center().y / fit.scale - root_top,
    );
    let center = origin + size * 0.5;
    UiTransform {
        translation: Val2::px(target.x - center.x, target.y - center.y),
        scale: Vec2::splat(scale),
        ..default()
    }
}

/// When a full-height panel cannot fit above a landscape IME, prioritize the
/// actual edited control rather than centering the whole panel behind it.
fn keep_local_y_above_ime(
    mut transform: UiTransform,
    origin: Vec2,
    panel_size: Vec2,
    local_y: f32,
    fit: CrystalStageTransform,
    viewport: Vec2,
    safe_edges: Vec4,
    root_top: f32,
) -> UiTransform {
    const IME_GUTTER: f32 = 8.0;
    let (Val::Px(dx), Val::Px(dy)) = (transform.translation.x, transform.translation.y) else {
        return transform;
    };
    let center = origin + panel_size * 0.5;
    let current_y = center.y + dy + transform.scale.y * (origin.y + local_y - center.y);
    let target_y = (viewport.y - safe_edges.w - IME_GUTTER) / fit.scale.max(0.01) - root_top;
    if current_y > target_y {
        transform.translation = Val2::px(dx, dy + target_y - current_y);
    }
    transform
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

fn player_editor_bottom(field: Option<&str>, _scale: f32) -> f32 {
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
        // Recipient is a preceding shared modal. Standalone letter/parcel
        // windows now fit their complete source geometry above the IME.
        Some("mail-recipient") => 411.0 + 8.0,
        Some("mail-message") => 750.0,
        Some("storage-password") => {
            let rect = mir2_client_bevy::crystal_ui::overlays::CRYSTAL_STORAGE_PANEL_RECT;
            rect.top + rect.height + 8.0
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
    if matches!(model.screen, Screen::StartingGame | Screen::InGame) {
        host.flush_personal();
        host.flush_npc(
            model.screen,
            player
                .as_deref()
                .is_some_and(|player| player.accepts_npc_service_reply()),
        );
    }
    #[cfg(target_os = "android")]
    if let Some(render) = crate::live_entity::poll_action_frame() {
        let _ = mir2_bevy_runtime::native_ingest::push_native_entity_render_state(render);
    }
    #[cfg(target_os = "android")]
    if let Some(event) = crate::world_assets::poll_packaged_map_atlas_load() {
        host.render_load_active = false;
        match event {
            crate::world_assets::PackagedMapAtlasLoadEvent::Ready {
                request_id,
                summary,
            } => {
                info!(
                    request_id,
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
                    entity_unindexed_rects = summary.entity_unindexed_rect_count,
                    entity_compressed_bytes = summary.entity_compressed_bytes,
                    entity_rgba_bytes = summary.entity_rgba_bytes,
                    entity_manifest_sha256 = summary.entity_manifest_sha256,
                    "packaged Android world frame queued"
                );
                if host.pending_render_request == Some(request_id)
                    && matches!(model.screen, Screen::StartingGame | Screen::InGame)
                {
                    model.notice = Some(ShellNotice::info(format!(
                        "World frame queued: {} map tiles and {} entity layers ({} unresolved map draws, {} unresolved entities).",
                        summary.map_tile_count + summary.map_standalone_tile_count,
                        summary.entity_layer_count,
                        summary.unresolved_draw_count,
                        summary.unresolved_entity_count,
                    )));
                }
            }
            crate::world_assets::PackagedMapAtlasLoadEvent::Failed {
                request_id,
                message,
            } => {
                warn!(request_id, %message, "packaged Android map atlas load failed");
                if fail_current_render_load(&mut host, &mut model, request_id, &message) {
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
                    intents.drain().for_each(drop);
                    if let Some(effects) = effects.as_deref_mut() {
                        discard_player_commands(effects);
                    }
                    OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
                    send(json!({"type":"disconnect"}));
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
            if matches!(host.phase.as_str(), "STARTING" | "IN_GAME")
                && matches!(model.screen, Screen::StartingGame | Screen::InGame)
            {
                if let Some(raw) = value["envelope"].as_str() {
                    // Large personal models use their dedicated bounded,
                    // owner-fenced decoders, not siblings' small packet caps.
                    let rejected = if crate::mail_ingress::is_mail_packet(raw) {
                        host.accept_mail_packet(model.screen, raw).is_err()
                    } else if crate::social_ingress::is_social_packet(raw) {
                        host.accept_social_packet(model.screen, raw).is_err()
                    } else {
                        host.accept_skill_packet(model.screen, raw).is_err()
                            || host.accept_inventory_packet(model.screen, raw).is_err()
                            || host.accept_player_packet(model.screen, raw).is_err()
                            || host.accept_chat_packet(model.screen, raw).is_err()
                            || host.accept_quest_packet(model.screen, raw).is_err()
                            || host.accept_game_shop_packet(model.screen, raw).is_err()
                            || host.accept_storage_packet(model.screen, raw).is_err()
                            || host
                                .accept_npc_packet(
                                    model.screen,
                                    raw,
                                    player
                                        .as_deref()
                                        .is_some_and(|player| player.accepts_npc_service_reply()),
                                )
                                .is_err()
                    };
                    if rejected {
                        host.reset_gameplay();
                        #[cfg(target_os = "android")]
                        crate::world_assets::cancel_packaged_map_atlas_load();
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
                        if let Some(effects) = effects.as_deref_mut() {
                            discard_player_commands(effects);
                        }
                        mir2_bevy_runtime::native_ingest::push_native_data_reset();
                        model.apply_gateway_event(Event::Disconnect {
                            reason: Some("Invalid or overflowing gameplay data; reconnect".into()),
                        });
                        intents.drain().for_each(drop);
                        OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        send(json!({"type":"disconnect"}));
                        continue;
                    }
                    host.flush_personal();
                    host.flush_npc(
                        model.screen,
                        player
                            .as_deref()
                            .is_some_and(|player| player.accepts_npc_service_reply()),
                    );
                }
            }
            #[cfg(target_os = "android")]
            if let Some(raw) = value["envelope"]
                .as_str()
                .filter(|_| {
                    host.phase == "IN_GAME"
                        && matches!(model.screen, Screen::StartingGame | Screen::InGame)
                })
                .and_then(scene_render_packet)
            {
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
                    host.reset_gameplay();
                    crate::world_assets::cancel_packaged_map_atlas_load();
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
                    crate::live_entity::LiveEntityPacketOutcome::Applied {
                        models,
                        render,
                        presentation_event,
                    } => {
                        if let Some(event) = presentation_event {
                            enqueue_presentation_event(event);
                        }
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
                        host.reset_gameplay();
                        crate::world_assets::cancel_packaged_map_atlas_load();
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
            if matches!(value["state"].as_str(), Some("pause" | "destroy")) {
                host.editor.close();
            }
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
            if !host.editor.accepts(&value) {
                continue;
            }
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
                Screen::Registration => {
                    model.apply_ui_intent(Intent::CancelRegistration);
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
            if !host.editor.accepts(&value) {
                continue;
            }
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
        // A stale Java world publication drained after a terminal decode error
        // must not reopen the host or its entity gate. StartingGame remains a
        // valid world phase while waiting for the exact NativeRenderReady.
        if matches!(phase, "STARTING" | "IN_GAME")
            && !matches!(model.screen, Screen::StartingGame | Screen::InGame)
        {
            continue;
        }
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
            host.reset_personal();
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
            host.skills.clear_scene();
            host.player.clear_scene();
            host.npc.clear_scene();
            host.quests.clear_scene();
            host.game_shop.clear_scene();
            host.storage.clear_scene();
            host.mail.clear_scene();
            host.social.clear_scene();
            if let Some(player) = player.as_deref_mut() {
                player.request_npc_service_exit();
            }
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
            let mut projected = crate::world_projection::project(
                raw,
                &world.map_file_name,
                &world.player_name,
                world.x,
                world.y,
            );
            if let Some(projection) = projected.as_mut() {
                if host.skills.snapshot(raw).is_err() || host.inventory.snapshot(raw).is_err() {
                    projected = None;
                } else if let Ok(ui) = host.player.snapshot(raw) {
                    if host.bind_chat_owner().is_ok()
                        && host.bind_social_owner().is_ok()
                        && host.bind_npc_snapshot(raw).is_ok()
                        && host.bind_quest_snapshot(raw).is_ok()
                        && host.bind_game_shop_snapshot(raw).is_ok()
                        && host.storage.snapshot(raw).is_ok()
                        && host.mail.snapshot(raw).is_ok()
                    {
                        projection.ui = ui;
                    } else {
                        projected = None;
                    }
                } else {
                    projected = None;
                }
            }
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
                host.reset_gameplay();
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
                model.apply_gateway_event(Event::Disconnect {
                    reason: Some("World snapshot rejected; reconnect".into()),
                });
                intents.drain().for_each(drop);
                OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
                send(json!({"type":"disconnect"}));
                continue;
            }
            host.flush_personal();
            host.flush_npc(
                model.screen,
                player
                    .as_deref()
                    .is_some_and(|player| player.accepts_npc_service_reply()),
            );
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
        if let Some(event) = host_account_event(&value) {
            model.apply_gateway_event(event);
        }
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
                // A rejected listed-character Start retires packet-first shop
                // metadata before another character can be selected.
                host.game_shop.reset();
                host.storage.reset();
                host.mail.reset();
                host.social.reset();
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

fn authenticated_render_character(model: &NativeShellModel) -> Option<CharacterSummary> {
    match model.screen {
        Screen::StartingGame => model
            .selected_character_index
            .and_then(|selected| {
                model
                    .characters
                    .iter()
                    .find(|character| character.index == selected)
            })
            .cloned(),
        Screen::InGame => model.active_character.clone(),
        _ => None,
    }
}

fn reject_current_render_receipt(
    host: &mut HostState,
    model: &mut NativeShellModel,
    request_id: u64,
    message: &str,
    intents: &mut NativeUiIntentQueue,
) -> bool {
    if !fail_current_render_load(host, model, request_id, message) {
        return false;
    }
    #[cfg(target_os = "android")]
    crate::world_assets::cancel_packaged_map_atlas_load();
    mir2_bevy_runtime::native_ingest::push_native_data_reset();
    intents.drain().for_each(drop);
    OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
    send(json!({"type":"disconnect"}));
    true
}

fn observe_render_receipt(
    mut host: ResMut<HostState>,
    mut model: ResMut<NativeShellModel>,
    receipt: Option<Res<mir2_bevy_runtime::native_render_receipt::NativeRenderReceipt>>,
    mut intents: ResMut<NativeUiIntentQueue>,
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
    let character = authenticated_render_character(&model);
    let Some(character) = character else {
        reject_current_render_receipt(
            &mut host,
            &mut model,
            request_id,
            "render completed without an authenticated character",
            &mut intents,
        );
        return;
    };
    if !render_receipt_matches_player(&ready, host.world.as_ref(), &character) {
        reject_current_render_receipt(
            &mut host,
            &mut model,
            request_id,
            "render receipt does not match the authenticated player",
            &mut intents,
        );
        return;
    }
    if model.screen == Screen::InGame {
        // Normal in-map snapshots refresh the renderer without replaying the
        // StartGame transition. They still have to match the active
        // authenticated character before the newest barrier is cleared.
        host.pending_render_request = None;
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
            host.reset_personal();
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
    let mut create_character_command_sent = false;
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
            Intent::CreateCharacter {
                name,
                class_name,
                gender_name,
            } if host.phase == "CHARACTERS"
                && model.create_character_request_in_flight
                && !create_character_command_sent =>
            {
                create_character_command_sent = true;
                send(json!({
                    "type":"createCharacter",
                    "name":name.trim(),
                    "className":class_name,
                    "genderName":gender_name,
                }));
            }
            Intent::ConfirmDeleteCharacter if host.phase == "CHARACTERS" => {
                if let Some(index) = model.delete_command_pending() {
                    model.mark_delete_command_sent();
                    send(json!({"type":"deleteCharacter","index":index}));
                }
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
            Intent::SubmitRegistration { .. } => {
                // The legacy Android transport has no full Crystal NewAccount
                // exchange. Never send a reduced account/password surrogate or
                // leave the shared modal locked waiting for a nonexistent reply.
                model.apply_gateway_event(Event::AccountCreationFailed {
                    message: "Account registration is not yet wired in the Android host.".into(),
                });
                model.registration.password.clear();
                model.registration.confirm_password.clear();
                model.registration.secret_question.clear();
                model.registration.secret_answer.clear();
            }
            Intent::SubmitChangePassword { .. }
            | Intent::CreateCharacter { .. }
            | Intent::ConfirmDeleteCharacter => {
                model.apply_gateway_event(Event::OperationFailure {
                    message: "This account operation is not wired in the Android UI milestone."
                        .into(),
                });
            }
            Intent::OpenChangePassword
            | Intent::CancelChangePassword
            | Intent::OpenRegistration
            | Intent::CancelRegistration
            | Intent::OpenSafeKey
            | Intent::CloseSafeKey
            | Intent::SafeKeyFocusAccount
            | Intent::SafeKeyFocusPassword
            | Intent::SafeKeyPress { .. }
            | Intent::SafeKeyDelete
            | Intent::SafeKeyRandom
            | Intent::OpenCharacterCreate
            | Intent::CancelCharacterCreate
            | Intent::DeleteCharacter { .. }
            | Intent::CancelDeleteCharacter
            | Intent::SelectCharacter { .. } => {}
        }
    }
}

/// Drain shared quest/object intents into Android's authenticated Gateway
/// producer. The authoritative snapshot remains unchanged until a server
/// packet or replacement snapshot arrives.
fn forward_quest_ui_intents(
    shell: Res<NativeShellModel>,
    windows: Query<&Window>,
    mut player: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
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
        || quest_state
            .as_deref()
            .is_some_and(mir2_client_bevy::quest_ui::QuestUiState::blocks_world_input)
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
            QuestUiIntent::InteractQuestNpc { .. }
            | QuestUiIntent::HarvestDirection { .. }
            | QuestUiIntent::ShareQuest { .. }
            | QuestUiIntent::PickUpTile => {
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
            .is_some_and(|gateway| gateway.enqueue(command.clone()).is_ok());
        if let Some(player) = player.as_deref_mut() {
            // Match Windows FIFO semantics. Exit remains latched even when
            // its transport queue is full; only an accepted new request may
            // reopen it, never an older retry or a delayed service reply.
            match &command {
                GatewayCommand::SelectNpcDialog { target }
                    if target.eq_ignore_ascii_case("@exit") =>
                {
                    player.request_npc_service_exit();
                }
                GatewayCommand::InteractNpc { .. } | GatewayCommand::SelectNpcDialog { .. }
                    if accepted =>
                {
                    player.begin_npc_service_request();
                }
                _ => {}
            }
        }
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

/// Forward only the seven shared native mail intents, after the existing
/// authenticated owner and render barriers. Never consume another UI domain.
fn forward_native_mail_ui_intents(
    mut shell: ResMut<NativeShellModel>,
    mut host: ResMut<HostState>,
    windows: Query<&Window>,
    lifecycle: Option<Res<crate::android_input::AndroidShellState>>,
    inventory: Option<Res<mir2_client_bevy::inventory::InventoryModel>>,
    mut intents: Option<ResMut<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntentQueue>>,
    mut pending: Option<ResMut<mir2_client_bevy::pending_operations::PendingOperations>>,
    mut gateway: Option<ResMut<crate::gateway_bridge::AndroidGatewayOutboundQueue>>,
    mut adapter: Option<ResMut<crate::gateway_bridge::AndroidGatewayHostAdapter>>,
    mut ui_state: Option<ResMut<mir2_ui_core::state::UiState>>,
) {
    let Some(intents) = intents.as_deref_mut() else {
        return;
    };
    if !matches!(shell.screen, Screen::StartingGame | Screen::InGame)
        || !matches!(host.phase.as_str(), "STARTING" | "IN_GAME")
    {
        for intent in intents.drain_mail_intents_bounded(usize::MAX) {
            if let (Some(pending), Some(key)) = (pending.as_deref_mut(), intent.pending_key()) {
                pending.release(&key); // Definitely unsent, never a server success.
            }
        }
        return;
    }
    // A same-owner scene load or a brief unfocused frame is not a new session.
    // Keep this bounded UI queue until ready; DataReset clears it on logout.
    let owner_ready = host.world.as_ref().is_some_and(|world| {
        host.player.identity().is_some_and(|(_, name)| name == world.player_name)
    });
    let ready = shell.screen == Screen::InGame
        && host.phase == "IN_GAME"
        && owner_ready
        && host.pending_world_request.is_none()
        && host.pending_render_request.is_none()
        && !host.render_load_active
        && host.deferred_render_load.is_none()
        && windows.single().is_ok_and(|window| window.focused)
        && lifecycle.as_deref().is_some_and(|state| {
            state.lifecycle == crate::android_input::AndroidLifecycle::Foreground
                && state.network == crate::android_input::AndroidNetwork::Available
        });
    if !ready {
        return;
    }
    for intent in intents.drain_mail_intents_bounded(16) {
        let accepted = mir2_client_bevy::native_mail_egress::project_native_mail_intent(
            &intent,
            inventory.as_deref(),
        )
        .ok()
        .is_some_and(|command| {
            gateway
                .as_deref_mut()
                .is_some_and(|queue| queue.enqueue_native_mail(&command).is_ok())
        });
        if accepted {
            continue;
        }
        // Quote/lock replies carry no request ID. Silently dropping an intent
        // would strand that shared reservation and make a retry ambiguous.
        // Use the existing connection/DataReset boundary, never invent an ACK.
        host.reset_gameplay();
        shell.apply_gateway_event(Event::Disconnect {
            reason: Some("Mail command could not be sent; reconnect".into()),
        });
        intents.clear();
        if let Some(queue) = gateway.as_deref_mut() {
            // Retire other domains while their correlation is still present.
            // Clearing the queue first would hide an outstanding password
            // change from the existing transport owner's unknown cleanup.
            if let (Some(adapter), Some(ui_state)) =
                (adapter.as_deref_mut(), ui_state.as_deref_mut())
            {
                adapter.on_connection_lost(queue, ui_state);
            } else {
                queue.mark_terminal_reset();
            }
        }
        crate::mir2_android_gateway_connection_lost();
        mir2_bevy_runtime::native_ingest::push_native_data_reset();
        OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).clear();
        send(json!({"type":"disconnect"}));
        return; // No later member of this drained batch can be sent.
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

/// Shared desktop editors request Winit IME in Update. On Android that request
/// focuses GameActivity's SurfaceView, stealing the input connection from our
/// field/epoch-guarded Java EditText. Keep Winit IME disabled before its Last
/// window sync; the existing Java host is the sole OS input connection owner.
fn keep_android_ime_owned_by_host(mut windows: Query<&mut Window>) {
    for mut window in &mut windows {
        if window.ime_enabled {
            window.ime_enabled = false;
        }
    }
}

fn keyboard(
    model: Res<NativeShellModel>,
    mut host: ResMut<HostState>,
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
    mut last_document: Local<Option<u64>>,
) {
    let sensitive = !model.login.account.is_empty()
        || !model.login.password.is_empty()
        || (model.screen == Screen::InGame && forms.field(&player).is_some_and(|v| v.2))
        || matches!(
            model.screen,
            Screen::ChangePassword | Screen::SafeKey | Screen::Registration
        );
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
    let document = field
        .filter(|v| v.0.starts_with("mail-"))
        .and_then(|_| forms.mail_draft_epoch());
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
        || (model.screen == Screen::InGame
            && field_name.is_some()
            && (field_name != *last_field || document != *last_document));
    if pressed {
        if let Some((field, text, password)) = field {
            let epoch = host.editor.open(field);
            send(
                json!({"type":"keyboard","field":field,"editorEpoch":epoch,"text":text,"password":password,"numeric":field.ends_with("amount"),"multiline":crate::text_input::is_multiline_editor(field)}),
            );
            *was_editing = true;
        } else if *was_editing {
            host.editor.close();
            send(json!({"type":"hideKeyboard"}));
            *was_editing = false;
        }
    } else if field.is_none() && *was_editing {
        host.editor.close();
        send(json!({"type":"hideKeyboard"}));
        *was_editing = false;
    }
    *last_field = field_name;
    *last_document = document;
}

#[cfg(test)]
#[path = "chat_editor_tests.rs"]
mod chat_editor_tests;

#[cfg(test)]
#[path = "npc_host_tests.rs"]
mod npc_host_tests;

#[cfg(test)]
#[path = "quest_host_tests.rs"]
mod quest_host_tests;

#[cfg(test)]
#[path = "mail_host_tests.rs"]
mod mail_host_tests;

#[cfg(test)]
#[path = "mail_egress_tests.rs"]
mod mail_egress_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn social_large_read_models_reside_without_entering_scene_decoders() {
        let event = game_shop_host_metadata("GuildNoticeChange", json!({
            "notice":(0..200).map(|_| "文".repeat(32)).collect::<Vec<_>>(),
            "update":0,"probe":"x".repeat(70_000)}));
        let raw = event["envelope"].as_str().unwrap();
        assert!(raw.len() > 65536 && raw.len() < 512 * 1024);
        let mut queue = VecDeque::new();
        enqueue_host_event(&mut queue, &event.to_string());
        assert_eq!(queue.front(), Some(&event), "Public social read model lost at host envelope cap");
        assert!(scene_render_packet(raw).is_none(), "Personal social data entered scene decoders");
    }

    #[test]
    fn social_host_actual_receive_stages_then_binds_without_render_acceptance() {
        let mut app=game_shop_receive_app();
        INBOX.lock().unwrap().extend([
            game_shop_host_metadata("GroupInvite",json!({"name":"Alice"})),
            game_shop_host_metadata("GuildStatus",json!({"guildName":"WireGuild","guildRankName":"Leader","myOptions":3})),
            game_shop_host_metadata("TradeAccept",json!({"name":"Alice"})),
            game_shop_host_metadata("TradeGold",json!({"amount":50})),
        ]);
        app.update();
        let host=app.world().resource::<HostState>();
        assert_eq!(host.social.pending_count(),4);
        assert_eq!(host.social.model(),&mir2_client_bevy::social::SocialModel::default());
        assert!(host.pending_render_request.is_none());
        INBOX.lock().unwrap().push_back(game_shop_host_world(42,"Fixture","0"));
        app.update();
        let host=app.world().resource::<HostState>();
        assert_eq!(host.social.pending_count(),0,"Typed native producer did not accept FIFO");
        assert_eq!(host.social.model().group.pending_invite_from.as_deref(),Some("Alice"));
        assert_eq!(host.social.model().guild.permissions,vec!["CanChangeRank","CanRecruit"]);
        assert_eq!(host.social.model().trade.partner_gold,50);
        assert_eq!(host.social.model().trade.my_gold,0);
        assert!(host.pending_render_request.is_some());
        assert_eq!(app.world().resource::<NativeShellModel>().screen,Screen::StartingGame);
        // This headless test proves the actual envelope/bind/queue producer,
        // not Android JNI, original overlay consumption, live WSS or rendering.
    }

    #[test]
    fn social_host_phase_and_render_failure_retire_owner_and_fifo() {
        let raw=game_shop_host_metadata("GroupInvite",json!({"name":"Alice"}));
        let raw=raw["envelope"].as_str().unwrap();
        let mut host=HostState::default();
        host.phase="STARTING".into();
        for screen in [Screen::Login,Screen::CharacterSelect,Screen::ConnectionLost] {
            assert!(!host.accept_social_packet(screen,raw).unwrap());
        }
        for phase in ["READY","CHARACTERS","DISCONNECTED"] {
            host.phase=phase.into();
            assert!(!host.accept_social_packet(Screen::StartingGame,raw).unwrap());
        }
        host.phase="STARTING".into();
        assert!(host.bind_social_owner().is_err());
        assert!(host.accept_social_packet(Screen::StartingGame,raw).unwrap());
        host.pending_render_request=Some(7);
        let mut model=NativeShellModel {screen:Screen::StartingGame,..default()};
        assert!(fail_current_render_load(&mut host,&mut model,7,"OFFLINE social fixture failure"));
        assert_eq!(host.social.pending_count(),0);
        assert_eq!(host.social.model(),&mir2_client_bevy::social::SocialModel::default());
        assert!(!host.social.flush(|_|panic!("Retired social FIFO leaked")));
        assert!(!host.accept_social_packet(Screen::StartingGame,raw).unwrap());
    }

    #[test]
    fn social_host_rejected_start_clears_packet_first_data_before_retry() {
        let mut app=game_shop_receive_app();
        INBOX.lock().unwrap().push_back(game_shop_host_metadata("AddMember",json!({"name":"Old"})));
        app.update();
        assert_eq!(app.world().resource::<HostState>().social.pending_count(),1);
        INBOX.lock().unwrap().push_back(json!({"phase":"CHARACTERS","message":"Rejected"}));
        app.update();
        assert_eq!(app.world().resource::<HostState>().social.pending_count(),0);
        let mut host=app.world_mut().resource_mut::<HostState>();
        host.phase="STARTING".into();
        drop(host);
        app.world_mut().resource_mut::<NativeShellModel>().screen=Screen::StartingGame;
        INBOX.lock().unwrap().extend([
            game_shop_host_metadata("AddMember",json!({"name":"New"})),
            game_shop_host_world(99,"Other","0"),
        ]);
        app.update();
        let host=app.world().resource::<HostState>();
        assert_eq!(host.social.model().group.members.len(),1);
        assert_eq!(host.social.model().group.members[0].name,"New");
        assert_eq!(host.social.pending_count(),0);
    }

    #[test]
    fn social_host_map_transition_keeps_personal_state_and_rejects_owner_swap() {
        let mut app=game_shop_receive_app();
        INBOX.lock().unwrap().extend([
            game_shop_host_world(42,"Fixture","0"),
            game_shop_host_metadata("GroupMemberInfo",json!({"members":[{"name":"Fixture"}],"leaderName":"Fixture"})),
            game_shop_host_metadata("GuildStatus",json!({"guildName":"WireGuild","guildRankName":"Leader","myOptions":255})),
        ]);
        app.update();
        INBOX.lock().unwrap().push_back(game_shop_host_world(42,"Fixture","1"));
        app.update();
        let host=app.world().resource::<HostState>();
        assert_eq!(host.social.model().group.members[0].name,"Fixture");
        assert_eq!(host.social.model().guild.name.as_deref(),Some("WireGuild"));
        assert_eq!(host.social.model().guild.permissions.len(),8);
        INBOX.lock().unwrap().push_back(game_shop_host_world(99,"Other","1"));
        app.update();
        assert_eq!(app.world().resource::<HostState>().social.model(),&mir2_client_bevy::social::SocialModel::default());
        assert_eq!(app.world().resource::<NativeShellModel>().screen,Screen::ConnectionLost);
    }

    #[test]
    fn social_host_invalid_large_model_revokes_terminal_batch_without_scene_bypass() {
        let mut app=game_shop_receive_app();
        let invalid=game_shop_host_metadata("GuildNoticeChange",json!({"notice":vec!["a";201]}));
        INBOX.lock().unwrap().extend([
            invalid,
            game_shop_host_metadata("AddMember",json!({"name":"Stale"})),
            game_shop_host_world(42,"Fixture","0"),
        ]);
        app.update();
        let host=app.world().resource::<HostState>();
        assert_eq!(host.phase,"DISCONNECTED");
        assert_eq!(host.social.pending_count(),0);
        assert!(host.pending_render_request.is_none());
        assert_eq!(app.world().resource::<NativeShellModel>().screen,Screen::ConnectionLost);
        let object=game_shop_host_metadata("ObjectGuildNameChanged",json!({"objectId":77,"guildName":"Peer"}));
        assert!(scene_render_packet(object["envelope"].as_str().unwrap()).is_some());
        assert!(scene_render_packet("{").is_some());
    }

    #[test]
    fn mail_scene_render_dispatch_excludes_private_results_and_full_mailbox() {
        use crate::scene_effects::{EffectPacketOutcome, SceneEffects};
        let own = json!({"type":"androidMailResult","packet":"ParcelCollected","result":1,
            "connectionGeneration":"9","ownerObjectId":42,"characterName":"Fixture",
            "claimMailId":"18446744073709551615"}).to_string();
        let full = game_shop_host_metadata("ReceiveMail", json!({"mail":(0..256).map(|index|
            json!({"mailId":index+1,"senderName":"NPC","message":"full mailbox".repeat(20),
                "gold":77,"items":[],"collected":false})).collect::<Vec<_>>()}));
        let full = full["envelope"].as_str().unwrap();
        assert!(full.len() > 16 * 1024 && full.len() <= crate::mail_ingress::MAX_MAIL_PACKET_BYTES);
        let positions = std::collections::HashMap::new();
        for raw in [&own, full] {
            assert!(crate::mail_ingress::is_mail_packet(raw));
            assert_eq!(SceneEffects::default().observe_packet(raw, 0, &positions), EffectPacketOutcome::Rejected);
            assert!(scene_render_packet(raw).is_none(), "Personal mail reached the Android-only scene decoders");
        }
        for packet in ["ReceiveMail", "MailSendRequest", "MailCost", "MailLockedItem"] {
            let raw = json!({"type":"packet","packet":packet,"payload":{}}).to_string();
            assert!(scene_render_packet(&raw).is_none());
        }
        // Classification alone grants no identity or admission. The original
        // receive path must still run phase/owner/epoch/schema checks first.
    }

    #[test]
    fn mail_scene_render_dispatch_keeps_non_mail_and_invalid_scene_checks() {
        use crate::scene_effects::{EffectPacketOutcome, SceneEffects};
        let positions = std::collections::HashMap::new();
        let movement = json!({"type":"packet","packet":"ObjectWalk",
            "payload":{"objectId":43,"x":302,"y":631,"direction":"Down"}}).to_string();
        let effect = json!({"type":"packet","packet":"MapEffect",
            "payload":{"location":{"x":302,"y":634},"effect":999,"value":0}}).to_string();
        for raw in [&movement, &effect] {
            assert_eq!(scene_render_packet(raw), Some(raw.as_str()));
            assert_eq!(SceneEffects::default().observe_packet(raw, 0, &positions), EffectPacketOutcome::Ignored);
        }
        for raw in ["{", r#"{"type":"unknown"}"#,
            r#"{"type":"packet","packet":"MapEffect","payload":{}}"#] {
            assert_eq!(scene_render_packet(raw), Some(raw));
            assert_eq!(SceneEffects::default().observe_packet(raw, 0, &positions), EffectPacketOutcome::Rejected);
        }
    }

    #[test]
    fn mail_result_actual_host_keeps_feedback_visible_and_reconciles_only_after_refresh() {
        use mir2_client_bevy::{mail::MailModel,pending_operations::PendingOperations,
            crystal_ui::overlays::{NativePlayerUiIntent,NativePlayerUiIntentQueue}};
        let mut app=game_shop_receive_app();
        app.add_plugins(mir2_bevy_runtime::Mir2NativeMailIngressPlugin)
            .init_resource::<NativePlayerUiIntentQueue>();
        let mut host_event=game_shop_host_world(42,"Fixture","0");
        let mut raw:Value=serde_json::from_str(host_event["worldSnapshot"].as_str().unwrap()).unwrap();
        raw["androidMailGeneration"]=json!("9");
        host_event["worldSnapshot"]=json!(raw.to_string());
        INBOX.lock().unwrap().push_back(host_event);app.update();
        let intent=NativePlayerUiIntent::ClaimMail{mail_id:17};
        let key=intent.pending_key().unwrap();
        app.world_mut().resource_scope(|world,mut pending:Mut<PendingOperations>|{
            assert!(world.resource_mut::<NativePlayerUiIntentQueue>().push_pending_intent(&mut pending,intent));
        });
        // Emulate the producer dequeue; a Sent write does not retire pending.
        app.world_mut().resource_mut::<NativePlayerUiIntentQueue>().drain_intents();
        let own=json!({"type":"androidMailResult","packet":"ParcelCollected","result":1,
            "connectionGeneration":"9","ownerObjectId":42,"characterName":"Fixture","claimMailId":"17"});
        INBOX.lock().unwrap().push_back(json!({"type":"gatewayGameplayPacket","envelope":own.to_string()}));
        app.update();
        assert!(app.world().resource::<PendingOperations>().contains(&key));
        assert!(app.world().resource::<MailModel>().operation_feedback().is_none());
        INBOX.lock().unwrap().extend([
            game_shop_host_metadata("ReceiveMail",json!({"mail":[{"mailId":"17","senderName":"NPC","message":"Authoritative","gold":77,"items":[],"collected":false}]})),
            game_shop_host_metadata("ReceiveMail",json!({"mail":[]})),
        ]);
        app.update();
        let mail=app.world().resource::<MailModel>();
        assert_eq!(mail.operation_feedback().unwrap().mail_id,Some(17));
        assert_eq!(mail.mails[0].gold,77);assert!(!mail.mails[0].claimed);
        assert!(!app.world().resource::<PendingOperations>().contains(&key));
        assert_eq!(app.world().resource::<NativeShellModel>().screen,Screen::StartingGame);
        app.update();
        assert!(app.world().resource::<MailModel>().mails.is_empty());
        assert_eq!(app.world().resource::<HostState>().phase,"IN_GAME");
    }

    #[test]
    fn storage_host_phase_gate_and_render_failure_retire_pending_metadata() {
        let raw = json!({"type":"packet","packet":"ResizeStorage","payload":{
            "size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9}})
        .to_string();
        let mut host = HostState::default();
        assert!(!host
            .accept_storage_packet(Screen::StartingGame, &raw)
            .unwrap());
        host.phase = "STARTING".into();
        for screen in [
            Screen::Login,
            Screen::CharacterSelect,
            Screen::ConnectionLost,
        ] {
            assert!(!host.accept_storage_packet(screen, &raw).unwrap());
        }
        assert!(host
            .accept_storage_packet(Screen::StartingGame, &raw)
            .unwrap());
        assert_eq!(host.storage.pending_count(), 1);
        host.pending_render_request = Some(7);
        let mut shell = NativeShellModel {
            screen: Screen::StartingGame,
            ..default()
        };
        assert!(fail_current_render_load(
            &mut host,
            &mut shell,
            7,
            "OFFLINE storage fixture failure"
        ));
        assert_eq!(host.storage.pending_count(), 0);
        assert!(!host.storage.flush(
            |_| panic!("retired model"),
            |_| panic!("retired items"),
            |_| panic!("retired patch")
        ));
    }

    #[test]
    fn storage_host_actual_receive_stages_then_admits_without_skipping_render_barrier() {
        let mut app = game_shop_receive_app();
        INBOX.lock().unwrap().extend([
            game_shop_host_metadata(
                "UserStorage",
                json!({"storage":[null,{"uniqueId":"17","name":"fixture"}]}),
            ),
            game_shop_host_metadata(
                "ResizeStorage",
                json!({"size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9}),
            ),
            game_shop_host_metadata(
                "StorageUnlockResult",
                json!({"result":2,"hasPassword":true}),
            ),
        ]);
        app.update();
        assert_eq!(
            app.world().resource::<HostState>().storage.pending_count(),
            3
        );
        assert!(app
            .world()
            .resource::<HostState>()
            .pending_render_request
            .is_none());
        assert!(app
            .world()
            .resource::<mir2_client_bevy::storage::StorageModel>()
            .items
            .is_empty());
        let mut snapshot = game_shop_host_world(42, "Fixture", "0");
        let mut raw: Value =
            serde_json::from_str(snapshot["worldSnapshot"].as_str().unwrap()).unwrap();
        raw["storageItems"] = json!([]);
        raw["hasStoragePassword"] = json!(true);
        raw["requireStoragePassword"] = json!(true);
        snapshot["worldSnapshot"] = json!(raw.to_string());
        INBOX.lock().unwrap().push_back(snapshot);
        app.update();
        let host = app.world().resource::<HostState>();
        assert_eq!(host.storage.pending_count(), 0);
        assert_eq!(host.phase, "IN_GAME");
        assert!(host.pending_render_request.is_some());
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::StartingGame
        );
        assert!(OUTBOX.lock().unwrap().is_empty());
    }

    #[test]
    fn storage_host_rejected_start_drops_metadata_before_another_character() {
        let mut app = game_shop_receive_app();
        INBOX.lock().unwrap().push_back(game_shop_host_metadata(
            "ResizeStorage",
            json!({"size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9}),
        ));
        app.update();
        assert_eq!(
            app.world().resource::<HostState>().storage.pending_count(),
            1
        );
        INBOX
            .lock()
            .unwrap()
            .push_back(json!({"phase":"CHARACTERS","message":"OFFLINE rejected Start"}));
        app.update();
        assert_eq!(
            app.world().resource::<HostState>().storage.pending_count(),
            0
        );
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::CharacterSelect
        );
        app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::StartingGame;
        app.world_mut().resource_mut::<HostState>().phase = "STARTING".into();
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(43, "Other", "1"));
        app.update();
        assert_eq!(app.world().resource::<HostState>().phase, "IN_GAME");
        assert_eq!(
            app.world().resource::<HostState>().storage.pending_count(),
            0
        );
    }

    #[test]
    fn storage_host_overflow_rejects_later_metadata_and_world_in_terminal_batch() {
        let mut app = game_shop_receive_app();
        app.world_mut()
            .resource_mut::<mir2_client_bevy::storage::StorageModel>()
            .items
            .push(mir2_client_bevy::inventory::ItemModel {
                unique_id: Some(17),
                slot: 0,
                container: 4,
                ..default()
            });
        let metadata = game_shop_host_metadata(
            "ResizeStorage",
            json!({"size":160,"hasExpandedStorage":true,"expiryTimeBinaryDatetime":9}),
        );
        INBOX
            .lock()
            .unwrap()
            .extend(std::iter::repeat_n(metadata, 33));
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(42, "Fixture", "0"));
        app.update();
        let host = app.world().resource::<HostState>();
        assert_eq!(host.phase, "DISCONNECTED");
        assert_eq!(host.storage.pending_count(), 0);
        assert!(host.world.is_none() && host.pending_render_request.is_none());
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::ConnectionLost
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::storage::StorageModel>()
            .items
            .is_empty());
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(command["type"], "disconnect");
        assert!(OUTBOX.lock().unwrap().is_empty());
    }

    #[test]
    fn storage_host_owner_switch_without_reset_revokes_old_contents() {
        let mut app = game_shop_receive_app();
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(42, "Fixture", "0"));
        app.update();
        app.world_mut()
            .resource_mut::<mir2_client_bevy::storage::StorageModel>()
            .items
            .push(mir2_client_bevy::inventory::ItemModel {
                unique_id: Some(17),
                slot: 0,
                container: 4,
                ..default()
            });
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(43, "Other", "0"));
        app.update();
        assert_eq!(app.world().resource::<HostState>().phase, "DISCONNECTED");
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::ConnectionLost
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::storage::StorageModel>()
            .items
            .is_empty());
    }

    fn game_shop_receive_app() -> App {
        INBOX.lock().unwrap().clear();
        OUTBOX.lock().unwrap().clear();
        let mut app = App::new();
        app.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
        mir2_bevy_runtime::native_ingest::install_native_ingestion(&mut app);
        #[cfg(feature = "ui-preview")]
        app.init_resource::<crate::ui_preview::PreviewRequest>();
        app.insert_resource(NativeShellModel {
            screen: Screen::StartingGame,
            ..default()
        })
        .insert_resource(HostState {
            phase: "STARTING".into(),
            ..default()
        })
        .init_resource::<NativeUiIntentQueue>()
        .add_systems(PreUpdate, receive);
        app
    }

    fn game_shop_host_metadata(packet: &str, payload: Value) -> Value {
        json!({"type":"gatewayGameplayPacket","envelope":
            json!({"type":"packet","packet":packet,"payload":payload}).to_string()})
    }

    fn game_shop_host_world(owner: u32, name: &str, map: &str) -> Value {
        let raw = json!({"playerObjectId":owner,"mapFileName":map,
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":name,"x":10,"y":20}]});
        json!({"phase":"IN_GAME","world":{
            "playerName":name,"mapFileName":map,"x":10,"y":20},
            "worldSnapshot":raw.to_string()})
    }

    #[test]
    fn game_shop_host_phase_gate_and_render_failure_retire_pending_metadata() {
        let raw =
            json!({"type":"packet","packet":"GameShopInfo","payload":{"gIndex":31}}).to_string();
        let mut host = HostState::default();
        for phase in ["DISCONNECTED", "READY", "CHARACTERS"] {
            host.phase = phase.into();
            assert!(!host
                .accept_game_shop_packet(Screen::StartingGame, &raw)
                .unwrap());
        }
        host.phase = "STARTING".into();
        for screen in [
            Screen::Login,
            Screen::CharacterSelect,
            Screen::ConnectionLost,
        ] {
            assert!(!host.accept_game_shop_packet(screen, &raw).unwrap());
        }
        assert!(host
            .accept_game_shop_packet(Screen::StartingGame, &raw)
            .unwrap());
        assert_eq!(host.game_shop.pending_count(), 1);
        host.pending_render_request = Some(7);
        let mut shell = NativeShellModel {
            screen: Screen::StartingGame,
            ..default()
        };
        assert!(fail_current_render_load(
            &mut host,
            &mut shell,
            7,
            "OFFLINE fixture failure"
        ));
        assert_eq!(host.game_shop.pending_count(), 0);
        assert!(!host
            .game_shop
            .flush(|_| panic!("retired catalog"), |_| panic!("retired stock")));
    }

    #[test]
    fn game_shop_host_actual_receive_stages_then_admits_without_skipping_render_barrier() {
        // Exercise the actual Java-envelope receive/bind/native-queue producer.
        // This headless fixture is not Android JNI, a rendered shop or live WSS.
        let mut app = game_shop_receive_app();
        INBOX.lock().unwrap().extend([
            game_shop_host_metadata("GameShopStock", json!({"gIndex":31,"stockLevel":3})),
            game_shop_host_metadata(
                "GameShopInfo",
                json!({"gIndex":31,"stock":10,"stockLevel":8}),
            ),
        ]);
        app.update();
        assert_eq!(
            app.world()
                .resource::<HostState>()
                .game_shop
                .pending_count(),
            2
        );
        assert!(app
            .world()
            .resource::<HostState>()
            .pending_render_request
            .is_none());
        assert!(app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>()
            .items
            .is_empty());
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(42, "Fixture", "0"));
        app.update();
        let host = app.world().resource::<HostState>();
        assert_eq!(
            host.game_shop.pending_count(),
            0,
            "producer admits deferred metadata after owner"
        );
        assert_eq!(host.phase, "IN_GAME");
        assert!(host.pending_render_request.is_some());
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::StartingGame
        );
        assert!(OUTBOX.lock().unwrap().is_empty());
    }

    #[test]
    fn game_shop_host_rejected_start_clears_staged_catalog_before_retry() {
        let mut app = game_shop_receive_app();
        INBOX.lock().unwrap().push_back(game_shop_host_metadata(
            "GameShopInfo",
            json!({"gIndex":31}),
        ));
        app.update();
        assert_eq!(
            app.world()
                .resource::<HostState>()
                .game_shop
                .pending_count(),
            1
        );
        INBOX
            .lock()
            .unwrap()
            .push_back(json!({"phase":"CHARACTERS","message":"OFFLINE rejected Start"}));
        app.update();
        assert_eq!(
            app.world()
                .resource::<HostState>()
                .game_shop
                .pending_count(),
            0
        );
        assert_eq!(app.world().resource::<HostState>().phase, "CHARACTERS");
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::CharacterSelect
        );
        app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::StartingGame;
        app.world_mut().resource_mut::<HostState>().phase = "STARTING".into();
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(43, "Other", "1"));
        app.update();
        assert_eq!(
            app.world()
                .resource::<HostState>()
                .game_shop
                .pending_count(),
            0
        );
        assert_eq!(app.world().resource::<HostState>().phase, "IN_GAME");
    }

    #[test]
    fn game_shop_host_terminal_batch_rejects_later_metadata_and_stale_world() {
        let mut app = game_shop_receive_app();
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(42, "Fixture", "0"));
        app.update();
        app.world_mut()
            .resource_mut::<mir2_client_bevy::game_shop::GameShopModel>()
            .upsert(mir2_client_bevy::game_shop::GameShopEntry {
                game_shop_index: 31,
                ..default()
            });
        INBOX.lock().unwrap().extend([
            game_shop_host_metadata("GameShopInfo", json!({"gIndex":-1})),
            game_shop_host_metadata("GameShopInfo", json!({"gIndex":32})),
            game_shop_host_world(42, "Fixture", "0"),
        ]);
        app.update();
        let host = app.world().resource::<HostState>();
        assert_eq!(host.phase, "DISCONNECTED");
        assert_eq!(host.game_shop.pending_count(), 0);
        assert!(host.world.is_none() && host.pending_render_request.is_none());
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::ConnectionLost
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>()
            .items
            .is_empty());
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(command["type"], "disconnect");
        assert!(OUTBOX.lock().unwrap().is_empty());
    }

    #[test]
    fn game_shop_host_owner_swap_without_personal_reset_revokes_old_catalog() {
        let mut app = game_shop_receive_app();
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(42, "Fixture", "0"));
        app.update();
        app.world_mut()
            .resource_mut::<mir2_client_bevy::game_shop::GameShopModel>()
            .upsert(mir2_client_bevy::game_shop::GameShopEntry {
                game_shop_index: 31,
                ..default()
            });
        INBOX
            .lock()
            .unwrap()
            .push_back(game_shop_host_world(43, "Other", "0"));
        app.update();
        assert_eq!(app.world().resource::<HostState>().phase, "DISCONNECTED");
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::ConnectionLost
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>()
            .items
            .is_empty());
    }

    #[test]
    fn android_host_does_not_let_desktop_ime_steal_its_editor_connection() {
        let mut app = App::new();
        app.init_resource::<HostState>()
            .add_systems(Update, |mut windows: Query<&mut Window>| {
                for mut window in &mut windows {
                    window.ime_enabled = true;
                }
            })
            .add_systems(PostUpdate, super::keep_android_ime_owned_by_host);
        let window = app.world_mut().spawn(Window::default()).id();
        let epoch = app
            .world_mut()
            .resource_mut::<HostState>()
            .editor
            .open("mail-message");
        for _ in 0..3 {
            app.update();
            assert!(!app.world().get::<Window>(window).unwrap().ime_enabled);
            assert!(app
                .world()
                .resource::<HostState>()
                .editor
                .accepts(&json!({"field":"mail-message", "editorEpoch":epoch})));
        }
    }

    #[test]
    fn editor_epoch_rejects_old_callbacks_when_the_same_mail_field_reopens() {
        let mut editor = HostEditorSession::default();
        let first = editor.open("mail-message");
        let old = json!({"field":"mail-message", "editorEpoch":first});
        assert!(editor.accepts(&old));
        editor.close();
        assert!(!editor.accepts(&old));
        let second = editor.open("mail-message");
        assert_ne!(first, second);
        assert!(!editor.accepts(&old));
        assert!(editor.accepts(&json!({"field":"mail-message", "editorEpoch":second})));
        assert!(!editor.accepts(&json!({"field":"password", "editorEpoch":second})));
        assert!(!editor.accepts(&json!({"field":"mail-message"})));
    }

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
    fn render_receipt_uses_only_the_authenticated_character_for_each_barrier() {
        let character = CharacterSummary::new(7, "Fixture", 12, "Wizard", "Female");
        let mut model = NativeShellModel {
            screen: Screen::StartingGame,
            characters: vec![character.clone()],
            selected_character_index: Some(character.index),
            ..default()
        };
        assert_eq!(
            super::authenticated_render_character(&model),
            Some(character.clone())
        );

        model.selected_character_index = Some(99);
        assert!(super::authenticated_render_character(&model).is_none());
        model.screen = Screen::InGame;
        assert!(super::authenticated_render_character(&model).is_none());
        model.active_character = Some(character.clone());
        assert_eq!(
            super::authenticated_render_character(&model),
            Some(character)
        );
        model.screen = Screen::ConnectionLost;
        assert!(super::authenticated_render_character(&model).is_none());
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
    fn render_load_failure_recovers_only_the_matching_authoritative_frame() {
        let character = CharacterSummary::new(7, "Fixture", 12, "Wizard", "Female");
        let mut model = NativeShellModel {
            screen: Screen::InGame,
            characters: vec![character.clone()],
            selected_character_index: Some(character.index),
            active_character: Some(character),
            ..default()
        };
        let mut host = HostState {
            phase: "IN_GAME".into(),
            world: Some(HostWorldPosition {
                player_name: "Fixture".into(),
                map_file_name: "0".into(),
                x: 302,
                y: 634,
            }),
            pending_world_request: Some(42),
            pending_render_request: Some(42),
            render_load_active: true,
            ..default()
        };

        assert!(!super::fail_current_render_load(
            &mut host,
            &mut model,
            41,
            "stale frame failed"
        ));
        assert_eq!(host.pending_render_request, Some(42));
        assert_eq!(model.screen, Screen::InGame);

        assert!(super::fail_current_render_load(
            &mut host,
            &mut model,
            42,
            "missing atlas"
        ));
        assert_eq!(host.phase, "DISCONNECTED");
        assert!(host.world.is_none());
        assert!(host.pending_world_request.is_none());
        assert!(host.pending_render_request.is_none());
        assert!(!host.render_load_active);
        assert!(host.deferred_render_load.is_none());
        assert_ne!(model.screen, Screen::InGame);
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
    fn chat_requires_game_phase_and_validated_owner_and_clears_on_render_failure() {
        let mut host = HostState::default();
        let raw = json!({"type":"packet","packet":"ObjectChat","payload":{
            "objectId":99,"text":"peer message","chatType":"Normal"}})
        .to_string();
        host.phase = "IN_GAME".into();
        assert!(!host.accept_chat_packet(Screen::InGame, &raw).unwrap());
        assert!(host.bind_chat_owner().is_err());
        host.player
            .snapshot(
                &json!({"playerObjectId":42,
            "entities":[{"objectId":42,"kind":"selfPlayer","name":"Fixture"}]})
                .to_string(),
            )
            .unwrap();
        host.bind_chat_owner().unwrap();
        for screen in [
            Screen::Login,
            Screen::OpeningLogin,
            Screen::CharacterSelect,
            Screen::ConnectionLost,
        ] {
            assert!(!host.accept_chat_packet(screen, &raw).unwrap());
        }
        for phase in ["READY", "CHARACTERS", "DISCONNECTED"] {
            host.phase = phase.into();
            assert!(!host.accept_chat_packet(Screen::InGame, &raw).unwrap());
        }
        host.phase = "STARTING".into();
        host.player.clear_scene();
        host.bind_chat_owner().unwrap();
        assert!(host.accept_chat_packet(Screen::StartingGame, &raw).unwrap());
        assert!(!host.chat.flush(|_| false));
        host.pending_render_request = Some(7);
        let mut model = NativeShellModel {
            screen: Screen::StartingGame,
            ..default()
        };
        assert!(fail_current_render_load(
            &mut host,
            &mut model,
            7,
            "fixture render failure"
        ));
        let mut lines = 0;
        assert!(host.chat.flush(|_| {
            lines += 1;
            true
        }));
        assert_eq!(lines, 0);
        host.phase = "IN_GAME".into();
        assert!(!host.accept_chat_packet(Screen::InGame, &raw).unwrap());
    }

    #[test]
    fn invalid_chat_batch_revokes_host_and_rejects_later_chat_or_stale_view() {
        INBOX.lock().unwrap().clear();
        OUTBOX.lock().unwrap().clear();
        let mut app = App::new();
        #[cfg(feature = "ui-preview")]
        app.init_resource::<crate::ui_preview::PreviewRequest>();
        let mut host = HostState {
            phase: "IN_GAME".into(),
            ..default()
        };
        host.player
            .snapshot(
                &json!({"playerObjectId":42,
            "entities":[{"objectId":42,"kind":"selfPlayer","name":"Fixture"}]})
                .to_string(),
            )
            .unwrap();
        host.bind_chat_owner().unwrap();
        app.insert_resource(NativeShellModel {
            screen: Screen::InGame,
            ..default()
        })
        .insert_resource(host)
        .init_resource::<NativeUiIntentQueue>()
        .add_systems(Update, receive);
        let mut inbox = INBOX.lock().unwrap();
        inbox.push_back(json!({"type":"gatewayGameplayPacket","envelope":
            json!({"type":"packet","packet":"Chat","payload":{"text":"wrong field"}}).to_string()}));
        inbox.push_back(json!({"phase":"IN_GAME","world":{
            "playerName":"Fixture","mapFileName":"0","x":302,"y":634}}));
        inbox.push_back(json!({"type":"gatewayGameplayPacket","envelope":
            json!({"type":"packet","packet":"ObjectChat","payload":{"objectId":99,"text":"stale"}}).to_string()}));
        drop(inbox);
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::ConnectionLost
        );
        let mut host = app.world_mut().resource_mut::<HostState>();
        assert_eq!(host.phase, "DISCONNECTED");
        assert!(host.world.is_none());
        let mut lines = 0;
        assert!(host.chat.flush(|_| {
            lines += 1;
            true
        }));
        assert_eq!(lines, 0);
        assert_eq!(
            serde_json::from_str::<Value>(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap()
                ["type"],
            "disconnect"
        );
        assert!(OUTBOX.lock().unwrap().is_empty());
    }

    #[test]
    fn player_packets_require_host_game_phase_and_reset_on_render_failure() {
        let mut host = HostState {
            phase: "IN_GAME".into(),
            ..default()
        };
        host.player
            .snapshot(
                &json!({"playerObjectId":42,"gold":100,
            "entities":[{"objectId":42,"kind":"selfPlayer","name":"Fixture"}]})
                .to_string(),
            )
            .unwrap();
        let packet =
            json!({"type":"packet","packet":"LoseGold","payload":{"objectId":42,"gold":1}})
                .to_string();
        for screen in [
            Screen::Login,
            Screen::OpeningLogin,
            Screen::CharacterSelect,
            Screen::ConnectionLost,
        ] {
            assert!(!host.accept_player_packet(screen, &packet).unwrap());
        }
        for phase in ["READY", "CHARACTERS", "DISCONNECTED"] {
            host.phase = phase.into();
            assert!(!host.accept_player_packet(Screen::InGame, &packet).unwrap());
        }
        host.phase = "STARTING".into();
        host.player.clear_scene();
        assert!(host
            .accept_player_packet(Screen::StartingGame, &packet)
            .unwrap());
        host.pending_render_request = Some(7);
        let mut model = NativeShellModel {
            screen: Screen::StartingGame,
            ..default()
        };
        assert!(fail_current_render_load(
            &mut host,
            &mut model,
            7,
            "fixture render failure"
        ));
        host.phase = "IN_GAME".into();
        assert!(!host.accept_player_packet(Screen::InGame, &packet).unwrap());
    }

    #[test]
    fn rejected_personal_batch_revokes_host_and_ignores_stale_world_publication() {
        for stale_view in [false, true] {
            INBOX.lock().unwrap().clear();
            OUTBOX.lock().unwrap().clear();
            let mut app = App::new();
            #[cfg(feature = "ui-preview")]
            app.init_resource::<crate::ui_preview::PreviewRequest>();
            let mut host = HostState {
                phase: "IN_GAME".into(),
                world: Some(HostWorldPosition {
                    player_name: "Fixture".into(),
                    map_file_name: "0".into(),
                    x: 302,
                    y: 634,
                }),
                pending_world_request: Some(7),
                pending_render_request: Some(7),
                render_load_active: true,
                ..default()
            };
            host.player
                .snapshot(
                    &json!({"playerObjectId":42,"gold":100,
                "entities":[{"objectId":42,"kind":"selfPlayer","name":"Fixture"}]})
                    .to_string(),
                )
                .unwrap();
            app.insert_resource(NativeShellModel {
                screen: Screen::InGame,
                ..default()
            })
            .insert_resource(host)
            .init_resource::<NativeUiIntentQueue>()
            .add_systems(Update, receive);
            let mut inbox = INBOX.lock().unwrap();
            inbox.push_back(json!({"type":"gatewayGameplayPacket","envelope":
                json!({"type":"packet","packet":"GainedGold","payload":{"amount":-1}}).to_string()}));
            if stale_view {
                inbox.push_back(json!({"phase":"IN_GAME","world":{
                    "playerName":"Fixture","mapFileName":"0","x":302,"y":634}}));
            }
            inbox.push_back(json!({"type":"gatewayGameplayPacket","envelope":
                json!({"type":"packet","packet":"ObjectWalk","payload":{"objectId":43,"direction":"Right"}}).to_string()}));
            drop(inbox);
            app.update();
            assert_eq!(
                app.world().resource::<NativeShellModel>().screen,
                Screen::ConnectionLost
            );
            let mut host = app.world_mut().resource_mut::<HostState>();
            assert_eq!(host.phase, "DISCONNECTED", "stale_view={stale_view}");
            assert!(host.world.is_none());
            assert!(host.pending_world_request.is_none());
            assert!(host.pending_render_request.is_none());
            assert!(host.deferred_render_load.is_none());
            assert!(!host.render_load_active);
            let (mut ui, mut wallet) = (0, 0);
            assert!(host.player.flush(
                |_| {
                    ui += 1;
                    true
                },
                |_| {
                    wallet += 1;
                    true
                }
            ));
            assert_eq!((ui, wallet), (0, 0));
            let command: Value =
                serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
            assert_eq!(command["type"], "disconnect");
            assert!(OUTBOX.lock().unwrap().is_empty());
        }
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
    fn inventory_receipts_survive_scene_loading_but_not_render_or_session_failure() {
        let mut host = HostState {
            phase: "IN_GAME".into(),
            ..default()
        };
        let world=json!({"playerObjectId":42,"entities":[{"objectId":42,"kind":"selfPlayer","name":"Fixture"}],
            "inventoryItems":[{"uniqueId":7,"slot":3,"quantity":2}]}).to_string();
        host.inventory.snapshot(&world).unwrap();
        let packet = json!({"type":"packet","packet":"DropItem","payload":{
            "uniqueId":7,"count":2,"heroInventory":false,"success":false}})
        .to_string();
        for screen in [Screen::Login, Screen::OpeningLogin, Screen::ConnectionLost] {
            assert!(!host.accept_inventory_packet(screen, &packet).unwrap());
        }
        host.phase = "DISCONNECTED".into();
        assert!(!host
            .accept_inventory_packet(Screen::InGame, &packet)
            .unwrap());
        host.phase = "STARTING".into();
        assert!(host
            .accept_inventory_packet(Screen::StartingGame, &packet)
            .unwrap());
        host.pending_render_request = Some(7);
        let mut model = NativeShellModel {
            screen: Screen::StartingGame,
            ..default()
        };
        assert!(fail_current_render_load(
            &mut host,
            &mut model,
            7,
            "fixture render failure"
        ));
        assert!(host
            .inventory
            .flush(|_| panic!("Old inventory"), |_| panic!("Old receipt")));
        host.phase = "IN_GAME".into();
        assert!(!host
            .accept_inventory_packet(Screen::InGame, &packet)
            .unwrap());
    }

    #[test]
    fn skill_host_phase_guard_and_render_failure_cannot_republish_previous_character() {
        let mut host = HostState {
            phase: "IN_GAME".into(),
            ..default()
        };
        host.skills.snapshot(&json!({"tick":100,"playerObjectId":42,
            "entities":[{"objectId":42,"kind":"selfPlayer","name":"Fixture"}],
            "knownSkills":[{"id":1,"spell":"FireBall","castKind":"target","hotkey":1,"delayMs":2200}]
        }).to_string()).unwrap();
        assert!(host.skills.flush(|_| true));
        let packet =
            json!({"type":"packet","packet":"Magic","payload":{"spell":"FireBall","cast":true}})
                .to_string();
        for screen in [Screen::Login, Screen::OpeningLogin, Screen::ConnectionLost] {
            assert!(!host.accept_skill_packet(screen, &packet).unwrap());
        }
        host.phase = "DISCONNECTED".into();
        assert!(!host.accept_skill_packet(Screen::InGame, &packet).unwrap());
        host.phase = "IN_GAME".into();
        assert!(host.accept_skill_packet(Screen::InGame, &packet).unwrap());
        host.phase = "STARTING".into();
        host.skills.clear_scene();
        assert!(host
            .accept_skill_packet(Screen::StartingGame, &packet)
            .unwrap());
        host.pending_render_request = Some(7);
        let mut model = NativeShellModel {
            screen: Screen::StartingGame,
            ..default()
        };
        assert!(fail_current_render_load(
            &mut host,
            &mut model,
            7,
            "fixture render failure"
        ));
        let mut emitted = 0;
        assert!(host.skills.flush(|_| {
            emitted += 1;
            true
        }));
        assert_eq!(emitted, 0);
        assert!(!host.accept_skill_packet(Screen::InGame, &packet).unwrap());
        host.phase = "STARTING".into();
        assert!(!host
            .accept_skill_packet(Screen::StartingGame, &packet)
            .unwrap());
    }

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

    fn node_rect(node: &Node) -> (f32, f32, f32, f32) {
        let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
            (node.left, node.top, node.width, node.height)
        else {
            panic!("shell bleed geometry must stay in logical pixels")
        };
        (left, top, width, height)
    }

    #[test]
    fn wide_phone_shell_bleed_fills_only_noninteractive_side_gutters() {
        let fit = CrystalStageTransform::fit(2340.0, 1080.0);
        let left = shell_bleed_node(AndroidShellBleedEdge::Left, fit, true);
        let right = shell_bleed_node(AndroidShellBleedEdge::Right, fit, true);
        let top = shell_bleed_node(AndroidShellBleedEdge::Top, fit, true);
        let bottom = shell_bleed_node(AndroidShellBleedEdge::Bottom, fit, true);

        assert_eq!(node_rect(&left), (0.0, 0.0, 320.0, 768.0));
        assert_eq!(node_rect(&right), (1344.0, 0.0, 320.0, 768.0));
        assert_eq!(left.display, Display::Flex);
        assert_eq!(right.display, Display::Flex);
        assert_eq!(top.display, Display::None);
        assert_eq!(bottom.display, Display::None);

        for edge in [
            AndroidShellBleedEdge::Left,
            AndroidShellBleedEdge::Right,
            AndroidShellBleedEdge::Top,
            AndroidShellBleedEdge::Bottom,
        ] {
            assert_eq!(
                shell_bleed_node(edge, fit, false).display,
                Display::None,
                "the world renderer owns the full viewport after StartGame"
            );
        }
    }

    #[test]
    fn tall_shell_bleed_fills_vertical_gutters_without_resizing_the_stage() {
        let fit = CrystalStageTransform::fit(768.0, 1024.0);
        let top = shell_bleed_node(AndroidShellBleedEdge::Top, fit, true);
        let bottom = shell_bleed_node(AndroidShellBleedEdge::Bottom, fit, true);
        let (_, top_y, top_width, top_height) = node_rect(&top);
        let (_, bottom_y, bottom_width, bottom_height) = node_rect(&bottom);

        assert!((top_y - 0.0).abs() < 0.001);
        assert!((top_width - 1024.0).abs() < 0.001);
        assert!((top_height - 298.666_66).abs() < 0.001);
        assert!((bottom_y - 1066.666_6).abs() < 0.001);
        assert!((bottom_width - 1024.0).abs() < 0.001);
        assert!((bottom_height - 298.666_66).abs() < 0.001);
        assert_eq!(top.display, Display::Flex);
        assert_eq!(bottom.display, Display::Flex);
        assert_eq!(
            shell_bleed_node(AndroidShellBleedEdge::Left, fit, true).display,
            Display::None
        );
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
    fn compact_chat_settings_gets_a_bounded_touch_scale_on_phone_viewports() {
        for (width, height) in [(891.0, 411.0), (731.0, 411.0), (610.0, 274.0)] {
            let fit = CrystalStageTransform::fit(width, height);
            let focus = mobile_focus_scale(fit, Vec2::new(224.0, 180.0), width, height, 3.2);
            assert!((focus - 3.2).abs() < 0.001, "{width}x{height}: {focus}");
            assert!(224.0 * fit.scale * focus <= width - 32.0 + 0.001);
            assert!(180.0 * fit.scale * focus <= height - 32.0 + 0.001);
        }
    }

    #[test]
    fn focus_scale_respects_safe_edges_and_never_shrinks_dialogs() {
        let phone = CrystalStageTransform::fit(891.0, 411.0);
        let focus = mobile_focus_scale(phone, Vec2::new(224.0, 180.0), 760.0, 360.0, 3.2);
        assert!(focus > 1.0 && focus <= 3.2);
        assert!(224.0 * phone.scale * focus <= 760.0 - 32.0 + 0.001);
        assert!(180.0 * phone.scale * focus <= 360.0 - 32.0 + 0.001);

        let desktop = CrystalStageTransform::fit(1024.0, 768.0);
        let desktop_focus =
            mobile_focus_scale(desktop, Vec2::new(900.0, 700.0), 1024.0, 768.0, 3.2);
        assert!(desktop_focus >= 1.0);
        assert!(900.0 * desktop_focus <= 1024.0 - 32.0 + 0.001);
        assert!(700.0 * desktop_focus <= 768.0 - 32.0 + 0.001);
    }

    #[test]
    fn mail_recipient_ime_uses_the_shared_prompt_coordinates() {
        assert_eq!(player_editor_bottom(Some("mail-recipient"), 1.0), 419.0);
        assert_eq!(player_editor_bottom(Some("mail-recipient"), 0.5), 419.0);
    }

    #[test]
    fn big_map_search_stays_above_landscape_ime() {
        let viewport = Vec2::new(1600.0, 720.0);
        let fit = CrystalStageTransform::fit(viewport.x, viewport.y);
        let origin = Vec2::new(132.0, 134.0);
        let size = Vec2::new(760.0, 500.0);
        let safe_edges = Vec4::new(0.0, 0.0, 0.0, 417.0);
        let root_top = fit.offset_y / fit.scale;
        let centered =
            mobile_focus_transform(fit, origin, size, viewport, safe_edges, root_top, 1.8);
        let focused = keep_local_y_above_ime(
            centered, origin, size, 488.0, fit, viewport, safe_edges, root_top,
        );
        let (Val::Px(centered_y), Val::Px(focused_y)) =
            (centered.translation.y, focused.translation.y)
        else {
            panic!("focus translation must stay in logical pixels")
        };
        assert!(focused_y < centered_y);
        let center = origin + size * 0.5;
        let search_bottom =
            (root_top + center.y + focused_y + focused.scale.y * (origin.y + 488.0 - center.y))
                * fit.scale;
        assert!(search_bottom <= viewport.y - safe_edges.w - 8.0 + 0.001);
    }

    #[test]
    fn focused_panel_bounds_stay_inside_phone_safe_viewports() {
        for (width, height) in [(891.0, 411.0), (731.0, 411.0), (610.0, 274.0)] {
            let fit = CrystalStageTransform::fit(width, height);
            let root_top = fit.offset_y / fit.scale;
            for (origin, size) in [
                (Vec2::ZERO, Vec2::new(316.0, 236.0)),
                (Vec2::new(150.0, 100.0), Vec2::new(640.0, 344.0)),
                (Vec2::new(382.0, 207.0), Vec2::new(259.0, 354.0)),
                (Vec2::new(0.0, 224.0), Vec2::new(242.0, 330.0)),
                (Vec2::new(0.0, 224.0), Vec2::new(360.0, 360.0)),
                (Vec2::ZERO, Vec2::new(440.0, 224.0)),
                (Vec2::new(562.0, 5.0), Vec2::new(312.0, 444.0)),
                (Vec2::new(164.0, 146.0), Vec2::new(696.0, 476.0)),
                (Vec2::new(132.0, 134.0), Vec2::new(760.0, 500.0)),
            ] {
                let is_bounded_large_panel = size == Vec2::new(242.0, 330.0)
                    || size == Vec2::new(360.0, 360.0)
                    || size == Vec2::new(696.0, 476.0)
                    || size == Vec2::new(760.0, 500.0);
                let max_scale = if is_bounded_large_panel {
                    1.8
                } else if size == Vec2::new(440.0, 224.0) {
                    2.2
                } else {
                    3.2
                };
                let transform = mobile_focus_transform(
                    fit,
                    origin,
                    size,
                    Vec2::new(width, height),
                    Vec4::ZERO,
                    root_top,
                    max_scale,
                );
                let (Val::Px(dx), Val::Px(dy)) = (transform.translation.x, transform.translation.y)
                else {
                    panic!("focus translation must stay in logical pixels")
                };
                let center = origin + size * 0.5 + Vec2::new(dx, dy);
                let half = size * transform.scale * 0.5;
                let screen_min = Vec2::new(
                    fit.offset_x + (center.x - half.x) * fit.scale,
                    (root_top + center.y - half.y) * fit.scale,
                );
                let screen_max = Vec2::new(
                    fit.offset_x + (center.x + half.x) * fit.scale,
                    (root_top + center.y + half.y) * fit.scale,
                );
                assert!(screen_min.x >= 16.0 - 0.001, "{width} {origin:?}");
                assert!(screen_min.y >= 16.0 - 0.001, "{height} {origin:?}");
                assert!(screen_max.x <= width - 16.0 + 0.001, "{width} {origin:?}");
                assert!(screen_max.y <= height - 16.0 + 0.001, "{height} {origin:?}");
                if size == Vec2::new(440.0, 224.0) {
                    assert!(transform.scale.x <= 2.2);
                    assert!(28.0 * fit.scale * transform.scale.y * 2.625 >= 55.0);
                } else if size == Vec2::new(242.0, 330.0) || size == Vec2::new(360.0, 360.0) {
                    assert!(transform.scale.x <= 1.8);
                    assert!(32.0 * fit.scale * transform.scale.y * 2.625 >= 48.0);
                }
            }
        }
    }

    #[test]
    fn phone_focus_expands_the_actual_shared_quest_panel_without_rewriting_it() {
        use mir2_client_bevy::quest_ui::*;
        let mut app = App::new();
        let mut shell = NativeShellModel::default();
        shell.screen = Screen::InGame;
        app.insert_resource(HostState::default())
            .insert_resource(shell)
            .insert_resource(mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState::default())
            .insert_resource(mir2_client_bevy::quest_model::NpcDialogModel::default())
            .insert_resource(UiScale(1.0))
            .insert_resource(mir2_client_bevy::crystal_ui::hud::CrystalBeltPresentation::default())
            .add_systems(PostUpdate, fit_stage.in_set(AndroidStageFit));
        crate::phone_quests::install(&mut app);
        app.world_mut().spawn(Window {
            resolution: bevy::window::WindowResolution::new(1600, 720),
            ..default()
        });
        app.world_mut().spawn((QuestUiRoot, Node::default()));
        let diary = app
            .world_mut()
            .spawn((
                QuestLogPanel,
                UiTransform::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(QUEST_DIARY_DESIGN_LEFT),
                    top: px(QUEST_DIARY_DESIGN_TOP),
                    width: px(QUEST_DIARY_DESIGN_WIDTH),
                    height: px(QUEST_DIARY_DESIGN_HEIGHT),
                    display: Display::Flex,
                    ..default()
                },
            ))
            .id();
        app.update();
        assert!(
            app.world().get::<UiTransform>(diary).unwrap().scale.x > 1.2,
            "The shared quest panel is still only scaled as part of the desktop canvas"
        );
        let node = app.world().get::<Node>(diary).unwrap();
        assert_eq!(node.left, px(QUEST_DIARY_DESIGN_LEFT));
        assert_eq!(node.width, px(QUEST_DIARY_DESIGN_WIDTH));
    }

    #[test]
    fn fit_stage_applies_group_focus_to_the_bounded_shared_social_panel() {
        let mut app = App::new();
        let mut shell = NativeShellModel::default();
        shell.screen = Screen::InGame;
        let mut player = mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player.core.panel = mir2_ui_core::state::UiPanel::Group;
        app.insert_resource(HostState::default())
            .insert_resource(shell)
            .insert_resource(player)
            .insert_resource(mir2_client_bevy::quest_model::NpcDialogModel::default())
            .insert_resource(UiScale(1.0))
            .insert_resource(mir2_client_bevy::crystal_ui::hud::CrystalBeltPresentation::default())
            .add_systems(Update, fit_stage);
        app.world_mut().spawn(Window {
            resolution: bevy::window::WindowResolution::new(1600, 720),
            ..default()
        });
        let social = app
            .world_mut()
            .spawn((
                mir2_client_bevy::crystal_ui::overlays::OverlaySocialFocus,
                UiTransform::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(mir2_client_bevy::crystal_ui::overlays::CRYSTAL_GROUP_PANEL_RECT.left),
                    top: px(mir2_client_bevy::crystal_ui::overlays::CRYSTAL_GROUP_PANEL_RECT.top),
                    width: px(
                        mir2_client_bevy::crystal_ui::overlays::CRYSTAL_GROUP_PANEL_RECT.width,
                    ),
                    height: px(
                        mir2_client_bevy::crystal_ui::overlays::CRYSTAL_GROUP_PANEL_RECT.height,
                    ),
                    display: Display::Flex,
                    ..default()
                },
            ))
            .id();

        app.update();

        let transform = app.world().get::<UiTransform>(social).unwrap();
        assert!(transform.scale.x > 1.0, "{transform:?}");
        assert_ne!(transform.translation, Val2::ZERO);
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
        assert_eq!(player_editor_bottom(Some("storage-password"), 1.0), 452.0);
    }

    #[test]
    fn standalone_mail_windows_fit_above_ime_without_rewriting_shared_nodes() {
        use mir2_client_bevy::crystal_ui::overlays::OverlayMailWindow;
        for height in [300.0, 384.0] {
            let mut app = App::new();
            app.insert_resource(HostState {
                ime_bottom: 350.0,
                ..default()
            })
            .insert_resource(NativeShellModel {
                screen: Screen::InGame,
                ..default()
            })
            .init_resource::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>()
            .init_resource::<mir2_client_bevy::quest_model::NpcDialogModel>()
            .init_resource::<UiScale>()
            .init_resource::<mir2_client_bevy::crystal_ui::hud::CrystalBeltPresentation>()
            .add_systems(Update, fit_stage);
            app.world_mut().spawn(Window {
                resolution: bevy::window::WindowResolution::new(2340, 1080),
                ..default()
            });
            let original = Node {
                position_type: PositionType::Absolute,
                left: px(100),
                top: px(100),
                width: px(236),
                height: px(height),
                ..default()
            };
            let panel = app
                .world_mut()
                .spawn((OverlayMailWindow, original.clone(), UiTransform::default()))
                .id();
            app.update();
            let node = app.world().get::<Node>(panel).unwrap();
            assert_eq!(node.left, original.left);
            assert_eq!(node.top, original.top);
            assert_eq!(node.width, original.width);
            assert_eq!(node.height, original.height);
            assert_ne!(
                *app.world().get::<UiTransform>(panel).unwrap(),
                UiTransform::default()
            );
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
        assert!(model.apply_ui_intent(Intent::OpenCharacterCreate));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(Intent::OpenCharacterCreate);
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().screen,
            Screen::CharacterCreate
        );
        assert!(OUTBOX.lock().unwrap().is_empty());
        let create = Intent::CreateCharacter {
            name: "NewHero".into(),
            class_name: "Taoist".into(),
            gender_name: "Female".into(),
        };
        assert!(app
            .world_mut()
            .resource_mut::<NativeShellModel>()
            .apply_ui_intent(create.clone()));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(create);
        app.update();
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(
            command,
            json!({"type":"createCharacter","name":"NewHero","className":"Taoist","genderName":"Female"})
        );
        INBOX.lock().unwrap().push_back(json!({
            "phase":"CHARACTERS",
            "message":"Character created. Select a character.",
            "characters":[
                {"index":9,"name":"NewHero","level":1,"className":"Taoist","genderName":"Female"},
                {"index":7,"name":"Fixture","level":12,"className":"Wizard","genderName":"Female"}
            ],
            "accountEvent":{
                "type":"characterCreated",
                "character":{"index":9,"name":"NewHero","level":1,"className":"Taoist","genderName":"Female"}
            }
        }));
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeShellModel>()
                .selected_character_index,
            Some(9)
        );
        assert!(app
            .world_mut()
            .resource_mut::<NativeShellModel>()
            .apply_ui_intent(Intent::DeleteCharacter { character_index: 9 }));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(Intent::DeleteCharacter { character_index: 9 });
        app.update();
        assert!(OUTBOX.lock().unwrap().is_empty());
        assert!(app
            .world_mut()
            .resource_mut::<NativeShellModel>()
            .apply_ui_intent(Intent::ConfirmDeleteCharacter));
        app.world_mut()
            .resource_mut::<NativeUiIntentQueue>()
            .push(Intent::ConfirmDeleteCharacter);
        app.update();
        let command: Value =
            serde_json::from_str(&OUTBOX.lock().unwrap().pop_front().unwrap()).unwrap();
        assert_eq!(command, json!({"type":"deleteCharacter","index":9}));
        INBOX.lock().unwrap().push_back(json!({
            "phase":"CHARACTERS",
            "message":"Character deleted.",
            "characters":[
                {"index":7,"name":"Fixture","level":12,"className":"Wizard","genderName":"Female"}
            ],
            "accountEvent":{"type":"characterDeleted","characterIndex":9}
        }));
        app.update();
        assert_eq!(
            app.world().resource::<NativeShellModel>().characters.len(),
            1
        );
        assert_eq!(
            app.world()
                .resource::<NativeShellModel>()
                .selected_character_index,
            Some(7)
        );
        let mut model = app.world_mut().resource_mut::<NativeShellModel>();
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
