//! Compile-time isolated, offline UI specimens. Never enabled in a normal APK.
use bevy::prelude::*;
use mir2_client_bevy::{
    crystal_ui::overlays::NativePlayerUiState,
    native_shell::{CharacterSummary, NativeShellModel, NativeShellScreen as Screen},
    read_model::UiReadModel,
};
use mir2_ui_core::state::{UiPanel, UiScreen};

pub const SCENES: &[&str] = &[
    "login",
    "empty-roster",
    "roster",
    "create",
    "password",
    "safekey",
    "delete-character",
    "connecting",
    "starting",
    "disconnected",
    "hud",
    "multitouch",
    "world-render",
    "inventory",
    "character",
    "skills",
    "quests",
    "quests-ingress",
    "quest-confirmation",
    "quest-alert",
    "options",
    "platform",
    "menu",
    "gameshop",
    "npcshop",
    "npcshop-sell",
    "npcshop-repair",
    "npcshop-srepair",
    "mail",
    "bigmap",
    "storage",
    "storage-locked",
    "group",
    "guild",
    "trade",
    "chat-settings",
    "npc",
    "npc-ingress",
    "death",
    "chat",
    "help",
    "inventory-amount",
    "mail-compose",
];

#[derive(Resource, Default)]
pub struct PreviewRequest {
    pub scene: Option<String>,
    remaining: u8,
}

#[derive(Resource, Default)]
struct OfflineNpcPreviewReceipt {
    scene: Option<String>,
    logged: bool,
}

pub fn install(app: &mut App) {
    app.init_resource::<PreviewRequest>()
        .init_resource::<OfflineNpcPreviewReceipt>()
        .add_systems(Update, report_world_render_ready)
        .add_systems(Update, report_owned_hero_mana_visible)
        .add_systems(Update, start_world_render_motion_specimen)
        .add_systems(Update, report_world_render_motion_pose)
        .add_systems(PostUpdate, (apply, report_npc_preview_consumer).chain());
}

fn report_npc_preview_consumer(
    mut receipt: ResMut<OfflineNpcPreviewReceipt>,
    state: Res<NativePlayerUiState>,
    shop: Res<mir2_client_bevy::shop::ShopModel>,
    map: Res<mir2_client_bevy::big_map::BigMapModel>,
) {
    use mir2_client_bevy::shop::NpcShopServiceMode;
    if receipt.logged || !state.npc_shop_open() {
        return;
    }
    let Some(scene) = receipt.scene.clone() else {
        return;
    };
    let expected = match scene.as_str() {
        "npcshop" => NpcShopServiceMode::Buy,
        "npcshop-sell" => NpcShopServiceMode::Sell,
        "npcshop-repair" => NpcShopServiceMode::Repair,
        "npcshop-srepair" => NpcShopServiceMode::SpecialRepair,
        _ => return,
    };
    if shop.service_mode != expected {
        return;
    }
    // This observes actual shared consumer/UI state, not a GPU frame, server
    // receipt or live account. Original screenshots are a separate gate.
    let first = shop.goods.first();
    info!(scene, mode = ?shop.service_mode, goods = shop.goods.len(),
        first_id = first.map(|good| good.unique_id).unwrap_or(0),
        icon_width = first.map(|good| good.icon_width).unwrap_or(0),
        icon_height = first.map(|good| good.icon_height).unwrap_or(0),
        map_epoch = map.reset_epoch, "ANDROID_NPC_PREVIEW_SHARED_CONSUMER_OPEN_NOT_LIVE");
    receipt.logged = true;
}

fn report_owned_hero_mana_visible(
    bars: Query<Entity, With<crate::entity_overlays::ActorManaBar>>,
    mut reported: Local<bool>,
) {
    if !*reported && !bars.is_empty() {
        info!(
            count = bars.iter().count(),
            "ANDROID_OWNED_HERO_MANA_VISIBLE"
        );
        *reported = true;
    }
}

fn start_world_render_motion_specimen(
    receipt: Res<mir2_bevy_runtime::native_render_receipt::NativeRenderReceipt>,
    time: Res<Time>,
    mut ready_since_ms: Local<Option<u64>>,
    mut started: Local<bool>,
) {
    if *started || receipt.ready_for(u64::MAX).is_none() {
        return;
    }
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let Some(ready_ms) = *ready_since_ms else {
        *ready_since_ms = Some(now_ms);
        return;
    };
    if now_ms.saturating_sub(ready_ms) < 1_500 {
        return;
    }
    // Exercise the production packet-presentation path after both the packed
    // scene and its actor atlases have had time to become visible. The
    // authoritative snapshot already owns the target tile; this offline event
    // contributes only a temporary screen-space glide from the prior tile.
    crate::shared_shell::enqueue_presentation_event(
        serde_json::json!({
            "type": "remoteMotion",
            "atMs": now_ms,
            "packet": "ObjectBackStep",
            "objectId": "9003",
            "fromX": 301,
            "fromY": 634,
            "toX": 299,
            "toY": 634,
            "direction": "Right",
            "mode": "backstep",
            "phaseCount": 8
        })
        .to_string(),
    );
    info!("ANDROID_REMOTE_BACKSTEP_PRESENTATION_STARTED");
    let attack = serde_json::json!({
        "type":"packet", "packet":"ObjectAttack", "payload":{
            "objectId":9001, "x":302, "y":634, "direction":"Down"
        }
    })
    .to_string();
    if apply_offline_entity_packet(&attack) {
        info!("ANDROID_ASSASSIN_ATTACK_PRESENTATION_STARTED");
    } else {
        warn!("offline Assassin action was not accepted by the native renderer");
    }
    let user_dash = serde_json::json!({
        "type":"packet", "packet":"UserDash", "payload":{
            "location":{"x":302,"y":634}, "direction":"Right"
        }
    })
    .to_string();
    if apply_offline_entity_packet(&user_dash) {
        info!("ANDROID_USER_DASH_PRESENTATION_APPLIED");
    } else {
        warn!("offline UserDash specimen was not accepted by the native renderer");
    }
    for (packet, marker) in [
        (
            serde_json::json!({
                "type":"packet", "packet":"Pushed", "payload":{
                    "location":{"x":302,"y":634}, "direction":"Right"
                }
            })
            .to_string(),
            "ANDROID_LOCAL_PUSHED_PRESENTATION_APPLIED",
        ),
        (
            serde_json::json!({
                "type":"packet", "packet":"UserBackStep", "payload":{
                    "location":{"x":300,"y":634}, "direction":"Left"
                }
            })
            .to_string(),
            "ANDROID_LOCAL_BACKSTEP_PRESENTATION_APPLIED",
        ),
        (
            serde_json::json!({
                "type":"packet", "packet":"UserAttackMove", "payload":{
                    "location":{"x":301,"y":634}, "direction":"Right"
                }
            })
            .to_string(),
            "ANDROID_LOCAL_ATTACK_MOVE_PRESENTATION_APPLIED",
        ),
        (
            serde_json::json!({
                "type":"packet", "packet":"UserDashAttack", "payload":{
                    "location":{"x":302,"y":634}, "direction":"Right"
                }
            })
            .to_string(),
            "ANDROID_LOCAL_DASH_ATTACK_PRESENTATION_APPLIED",
        ),
    ] {
        if apply_offline_entity_packet(&packet) {
            info!("{marker}");
        } else {
            warn!(%marker, "offline local movement-skill specimen was rejected");
        }
    }
    let archer = serde_json::json!({
        "type":"packet", "packet":"ObjectRangeAttack", "payload":{
            "objectId":9004, "x":301, "y":631, "direction":"Right"
        }
    })
    .to_string();
    if apply_offline_entity_packet(&archer) {
        info!("ANDROID_ARCHER_RANGE_PRESENTATION_STARTED");
    } else {
        warn!("offline Archer action was not accepted by the native renderer");
    }
    let dash = serde_json::json!({
        "type":"packet", "packet":"ObjectDash", "payload":{
            "objectId":9004, "location":{"x":302,"y":631}, "direction":"Right"
        }
    })
    .to_string();
    if apply_offline_entity_packet(&dash) {
        info!("ANDROID_OBJECT_DASH_PRESENTATION_APPLIED");
    } else {
        warn!("offline ObjectDash specimen was not accepted by the native renderer");
    }
    let mounted = serde_json::json!({
        "type":"packet", "packet":"ObjectAttack", "payload":{
            "objectId":9005, "x":304, "y":631, "direction":"Down"
        }
    })
    .to_string();
    if apply_offline_entity_packet(&mounted) {
        info!("ANDROID_MOUNTED_ATTACK_PRESENTATION_STARTED");
    } else {
        warn!("offline mounted action was not accepted by the native renderer");
    }
    let pushed = serde_json::json!({
        "type":"packet", "packet":"ObjectPushed", "payload":{
            "objectId":9002, "location":{"x":303,"y":634}, "direction":"Left"
        }
    })
    .to_string();
    if apply_offline_entity_packet(&pushed) {
        info!("ANDROID_OBJECT_PUSHED_PRESENTATION_APPLIED");
    } else {
        warn!("offline ObjectPushed specimen was not accepted by the native renderer");
    }
    *started = true;
}

fn apply_offline_entity_packet(packet: &str) -> bool {
    match crate::live_entity::apply_packet(packet) {
        crate::live_entity::LiveEntityPacketOutcome::Applied {
            models,
            render,
            presentation_event,
        } => {
            let models_ready =
                mir2_bevy_runtime::native_ingest::push_native_entity_model_set(models);
            let render_ready = render
                .map(mir2_bevy_runtime::native_ingest::push_native_entity_render_state)
                .unwrap_or(false);
            let ready = models_ready && render_ready;
            if ready {
                if let Some(event) = presentation_event {
                    crate::shared_shell::enqueue_presentation_event(event);
                }
            }
            ready
        }
        crate::live_entity::LiveEntityPacketOutcome::Ignored
        | crate::live_entity::LiveEntityPacketOutcome::Rejected => false,
    }
}

fn apply_offline_object_metadata_specimen(
    overlays: &mut crate::entity_overlays::ActorOverlayModel,
) -> bool {
    let packets = [
        serde_json::json!({
            "type": "packet", "packet": "ObjectName", "payload": {
                "objectId": 9003, "name": "Packet renamed player"
            }
        })
        .to_string(),
        serde_json::json!({
            "type": "packet", "packet": "ObjectColourChanged", "payload": {
                "objectId": 9003, "nameColourArgb": -65281
            }
        })
        .to_string(),
        serde_json::json!({
            "type": "packet", "packet": "ObjectGuildNameChanged", "payload": {
                "objectId": 9003, "guildName": "AUTHORITATIVE"
            }
        })
        .to_string(),
    ];
    let mut models = None;
    for packet in packets {
        let crate::live_entity::LiveEntityPacketOutcome::Applied {
            models: next,
            render: None,
            ..
        } = crate::live_entity::apply_packet(&packet)
        else {
            return false;
        };
        models = Some(next);
    }
    let Some(models) = models else {
        return false;
    };
    let Some((center_x, center_y)) = overlays.center() else {
        return false;
    };
    let Some(projected) = crate::entity_overlays::project(&models, center_x, center_y) else {
        return false;
    };
    if !mir2_bevy_runtime::native_ingest::push_native_entity_model_set(models) {
        return false;
    }
    overlays.replace(projected);
    true
}

fn apply_offline_object_poison_specimen(
    overlays: &mut crate::entity_overlays::ActorOverlayModel,
) -> bool {
    let packet = serde_json::json!({
        "type": "packet", "packet": "ObjectPoisoned", "payload": {
            "objectId": 9003, "poison": 8
        }
    })
    .to_string();
    let crate::live_entity::LiveEntityPacketOutcome::Applied {
        models,
        render: Some(render),
        ..
    } = crate::live_entity::apply_packet(&packet)
    else {
        return false;
    };
    let Some((center_x, center_y)) = overlays.center() else {
        return false;
    };
    let Some(projected) = crate::entity_overlays::project(&models, center_x, center_y) else {
        return false;
    };
    if !mir2_bevy_runtime::native_ingest::push_native_entity_model_set(models)
        || !mir2_bevy_runtime::native_ingest::push_native_entity_render_state(render)
    {
        return false;
    }
    overlays.replace(projected);
    true
}

fn apply_offline_object_hidden_specimen(
    overlays: &mut crate::entity_overlays::ActorOverlayModel,
) -> bool {
    let packet = serde_json::json!({
        "type": "packet", "packet": "ObjectHidden", "payload": {
            "objectId": 9003, "hidden": true
        }
    })
    .to_string();
    let crate::live_entity::LiveEntityPacketOutcome::Applied {
        models,
        render: Some(render),
        ..
    } = crate::live_entity::apply_packet(&packet)
    else {
        return false;
    };
    let Some((center_x, center_y)) = overlays.center() else {
        return false;
    };
    let Some(projected) = crate::entity_overlays::project(&models, center_x, center_y) else {
        return false;
    };
    if !mir2_bevy_runtime::native_ingest::push_native_entity_model_set(models)
        || !mir2_bevy_runtime::native_ingest::push_native_entity_render_state(render)
    {
        return false;
    }
    overlays.replace(projected);
    true
}

fn apply_offline_owned_hero_vitals_specimen(
    overlays: &mut crate::entity_overlays::ActorOverlayModel,
) -> bool {
    let packets = [
        serde_json::json!({
            "type": "packet", "packet": "ObjectHealth", "payload": {
                "objectId": 9006, "percent": 82, "expire": 0
            }
        })
        .to_string(),
        serde_json::json!({
            "type": "packet", "packet": "ObjectMana", "payload": {
                "objectId": 9006, "percent": 64
            }
        })
        .to_string(),
    ];
    let mut latest_models = None;
    for packet in packets {
        let crate::live_entity::LiveEntityPacketOutcome::Applied {
            models,
            render,
            presentation_event: _,
        } = crate::live_entity::apply_packet(&packet)
        else {
            return false;
        };
        if !mir2_bevy_runtime::native_ingest::push_native_entity_model_set(models.clone()) {
            return false;
        }
        if render.is_some_and(|render| {
            !mir2_bevy_runtime::native_ingest::push_native_entity_render_state(render)
        }) {
            return false;
        }
        latest_models = Some(models);
    }
    let Some(models) = latest_models else {
        return false;
    };
    let Some((center_x, center_y)) = overlays.center() else {
        return false;
    };
    let Some(projected) = crate::entity_overlays::project(&models, center_x, center_y) else {
        return false;
    };
    overlays.replace(projected);
    true
}

fn report_world_render_motion_pose(
    poses: Res<mir2_bevy_runtime::PresentationPoseBuffer>,
    time: Res<Time>,
    _main_thread: NonSend<crate::shared_shell::AndroidPresentationMainThread>,
    mut saw_active: Local<bool>,
    mut saw_settled: Local<bool>,
    mut saw_pushed_active: Local<bool>,
    mut saw_pushed_settled: Local<bool>,
    mut saw_dash_active: Local<bool>,
    mut saw_dash_settled: Local<bool>,
    mut saw_local_skill_active: Local<bool>,
    mut saw_local_skill_settled: Local<bool>,
    mut reported_diagnostics: Local<bool>,
) {
    if !*reported_diagnostics && time.elapsed().as_millis() >= 4_500 {
        let diagnostics = mir2_bevy_runtime::get_mir2_remote_motion_presentation_diagnostics();
        info!(%diagnostics, "ANDROID_REMOTE_BACKSTEP_DIAGNOSTICS");
        *reported_diagnostics = true;
    }
    if let Some((x, y)) = poses.native_overlay_entity_offset("9003") {
        if !*saw_active && (x.abs() > f32::EPSILON || y.abs() > f32::EPSILON) {
            info!(
                offset_x = x,
                offset_y = y,
                "ANDROID_REMOTE_BACKSTEP_POSE_ACTIVE"
            );
            *saw_active = true;
        } else if *saw_active && !*saw_settled && x.abs() <= f32::EPSILON && y.abs() <= f32::EPSILON
        {
            info!("ANDROID_REMOTE_BACKSTEP_POSE_SETTLED");
            *saw_settled = true;
        }
    }
    if let Some((x, y)) = poses.native_overlay_entity_offset("9002") {
        if !*saw_pushed_active && (x.abs() > f32::EPSILON || y.abs() > f32::EPSILON) {
            info!(
                offset_x = x,
                offset_y = y,
                "ANDROID_OBJECT_PUSHED_POSE_ACTIVE"
            );
            *saw_pushed_active = true;
        } else if *saw_pushed_active
            && !*saw_pushed_settled
            && x.abs() <= f32::EPSILON
            && y.abs() <= f32::EPSILON
        {
            info!("ANDROID_OBJECT_PUSHED_POSE_SETTLED");
            *saw_pushed_settled = true;
        }
    }
    if let Some((x, y)) = poses.native_overlay_entity_offset("9004") {
        if !*saw_dash_active && (x.abs() > f32::EPSILON || y.abs() > f32::EPSILON) {
            info!(
                offset_x = x,
                offset_y = y,
                "ANDROID_OBJECT_DASH_POSE_ACTIVE"
            );
            *saw_dash_active = true;
        } else if *saw_dash_active
            && !*saw_dash_settled
            && x.abs() <= f32::EPSILON
            && y.abs() <= f32::EPSILON
        {
            info!("ANDROID_OBJECT_DASH_POSE_SETTLED");
            *saw_dash_settled = true;
        }
    }
    let local_skill_active =
        crate::live_entity::active_action_name(9001).as_deref() == Some("dashAttack");
    if local_skill_active && !*saw_local_skill_active {
        info!("ANDROID_LOCAL_MOVEMENT_SKILL_ACTION_ACTIVE");
        *saw_local_skill_active = true;
    } else if *saw_local_skill_active && !local_skill_active && !*saw_local_skill_settled {
        info!("ANDROID_LOCAL_MOVEMENT_SKILL_ACTION_SETTLED");
        *saw_local_skill_settled = true;
    }
}

fn report_world_render_ready(
    receipt: Res<mir2_bevy_runtime::native_render_receipt::NativeRenderReceipt>,
    time: Res<Time>,
    mut overlays: ResMut<crate::entity_overlays::ActorOverlayModel>,
    mut scene_effects: ResMut<crate::scene_effects::SceneEffects>,
    mut reported: Local<bool>,
) {
    if *reported {
        return;
    }
    if let Some(ready) = receipt.ready_for(u64::MAX) {
        // Start the offline-only damage specimen after the packaged world is
        // render-ready. Atlas decoding can otherwise consume the complete
        // production-length floater lifetime before the first visible frame.
        let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
        if apply_offline_object_metadata_specimen(&mut overlays) {
            info!("ANDROID_OBJECT_METADATA_PRESENTATION_APPLIED");
        } else {
            warn!("offline object metadata specimen was not accepted");
        }
        if apply_offline_object_poison_specimen(&mut overlays) {
            info!("ANDROID_OBJECT_POISON_PRESENTATION_APPLIED");
        } else {
            warn!("offline object poison specimen was not accepted");
        }
        if apply_offline_object_hidden_specimen(&mut overlays) {
            info!("ANDROID_OBJECT_HIDDEN_PRESENTATION_APPLIED");
        } else {
            warn!("offline ObjectHidden specimen was not accepted");
        }
        if apply_offline_owned_hero_vitals_specimen(&mut overlays) {
            info!("ANDROID_OWNED_HERO_VITALS_APPLIED");
        } else {
            warn!("offline owned-Hero health/mana specimen was not accepted");
        }
        overlays.observe_damage_events(
            [crate::live_entity::LiveDamageEvent {
                sequence: 1,
                object_id: 9002,
                damage: 128,
                damage_type: 2,
            }],
            now_ms,
        );
        let positions = overlays.actor_positions();
        // Offline-only exact-frame specimens. These enter the same bounded
        // packet projection and shared runtime renderer as live packets, but
        // are never evidence of a Gateway, account, or combat result.
        let barrier = serde_json::json!({
            "type":"packet", "packet":"ObjectEffect", "payload":{
                "objectId":9002, "effect":13, "effectType":0,
                "delayTime":0, "time":0
            }
        })
        .to_string();
        let fire_wall = serde_json::json!({
            "type":"packet", "packet":"ObjectSpell", "payload":{
                "objectId":9201, "location":{"x":300,"y":633},
                "spell":39, "direction":"Down", "param":0
            }
        })
        .to_string();
        let _ = scene_effects.observe_packet(&barrier, now_ms, &positions);
        let _ = scene_effects.observe_packet(&fire_wall, now_ms, &positions);
        info!(
            request_id = ready.request_id,
            center_x = ready.center_x,
            center_y = ready.center_y,
            map_tiles = ready.map_tile_count,
            entities = ready.entity_count,
            entity_layers = ready.entity_layer_count,
            "ANDROID_WORLD_RENDER_READY"
        );
        *reported = true;
    }
}

fn apply(world: &mut World) {
    let (scene, remaining) = {
        let mut request = world.resource_mut::<PreviewRequest>();
        let Some(scene) = request.scene.clone() else {
            return;
        };
        let remaining = request.remaining;
        request.remaining = remaining.saturating_add(1);
        if remaining >= 3 {
            request.scene = None;
            request.remaining = 0;
        }
        (scene, remaining)
    };
    if remaining == 0 {
        let mut receipt = world.resource_mut::<OfflineNpcPreviewReceipt>();
        receipt.scene = None;
        receipt.logged = false;
        // UI fixtures are not Gateway events and do not invoke auth/StartGame.
        let mut shell = NativeShellModel::default();
        shell.screen = match scene.as_str() {
            "login" => Screen::Login,
            "empty-roster" | "roster" => Screen::CharacterSelect,
            "create" => Screen::CharacterCreate,
            "password" => Screen::ChangePassword,
            "safekey" => Screen::SafeKey,
            "delete-character" => Screen::DeleteConfirm { index: 7 },
            "connecting" => Screen::Connecting,
            "starting" => Screen::StartingGame,
            "disconnected" => Screen::ConnectionLost,
            _ => Screen::InGame,
        };
        if scene != "empty-roster" {
            shell.characters = vec![
                CharacterSummary::new(7, "UI Warrior", 22, "Warrior", "Male"),
                CharacterSummary::new(12, "UI Wizard", 18, "Wizard", "Female"),
                CharacterSummary::new(21, "UI Taoist", 15, "Taoist", "Male"),
            ];
            shell.selected_character_index = Some(7);
        }
        if shell.screen == Screen::InGame {
            shell.active_character = shell.characters.first().cloned();
        }
        world.insert_resource(shell);
        world.spawn((
            Text::new(format!("OFFLINE UI PREVIEW — {scene} — NOT LIVE GAMEPLAY")),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.85, 0.2)),
            BackgroundColor(Color::BLACK),
            GlobalZIndex(10000),
            Node {
                position_type: PositionType::Absolute,
                left: px(8),
                top: px(8),
                ..default()
            },
        ));
    }
    if remaining != 3 {
        return;
    } // let normal session-boundary clearing run first
    let panel = preview_panel_for_scene(&scene);
    let mut state = world.resource_mut::<NativePlayerUiState>();
    state.core.screen = UiScreen::InGame;
    initialize_player_panel(&mut state, panel);
    if is_npc_service_preview(&scene) {
        begin_offline_npc_preview_request(&mut state);
    }
    if scene == "chat" {
        state.set_chat_focused(true);
    }
    if scene == "help" {
        state.help.open = true;
    }
    let mut model = world.resource_mut::<UiReadModel>();
    model.player.name = Some("OFFLINE UI FIXTURE".into());
    model.player.level = 22;
    model.player.hp = if scene == "death" { 0 } else { 160 };
    model.player.max_hp = 200;
    model.player.mp = 60;
    model.player.max_mp = 100;
    model.player.max_weight = 100;
    model.player.gold = 12345;
    model.player.credit = 500;
    if matches!(scene.as_str(), "hud" | "multitouch" | "world-render") {
        // Offline presentation specimen only, never a Gateway bootstrap.
        model.player.map_name = Some("BichonProvince".into());
    }
    drop(model);
    if matches!(scene.as_str(), "hud" | "multitouch" | "world-render") {
        let mut map = world.resource_mut::<mir2_client_bevy::map::MapModel>();
        map.center_x = if scene == "world-render" { 302 } else { 320 };
        map.center_y = if scene == "world-render" { 634 } else { 43 };
    }
    #[cfg(target_os = "android")]
    if scene == "world-render" {
        let scene = crate::world_projection::ProjectedScene {
            map_file_name: "0".into(),
            center_x: 302,
            center_y: 634,
            width: 22,
            height: 18,
        };
        let snapshot = serde_json::json!({
            "playerObjectId":"9001",
            "entities":[
                {"objectId":"9001","kind":"selfPlayer","classKey":"assassin","name":"OFFLINE UI FIXTURE","guildName":"CODEX","nameColourArgb":-256,"x":302,"y":634,"direction":"Down",
                 "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4,
                    "altBodyLibrary":"AArmour/00","altHairLibrary":"AHair/00",
                    "altWeaponLibrary":"AWeapon/00 R","altWeaponLibrarySecondary":"AWeapon/00 L",
                    "altFrameBaseOffset":0,"altWeaponFrameOffset":0}},
                {"objectId":"9002","kind":"monster","name":"Offline_monster","nameColourArgb":-65536,"x":304,"y":634,"direction":"Right","_healthPercent":65,"_healthExpireSeconds":90,"_healthGeneration":1,"_healthRevision":1,
                 "sprite":{"bodyLibrary":"Monster/003","frameBaseOffset":0,"directionStride":4}},
                {"objectId":"9003","kind":"player","name":"Motion witness","guildName":"BACKSTEP","nameColourArgb":-16711681,"x":299,"y":634,"direction":"Right",
                 "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4}},
                {"objectId":"9004","kind":"player","classKey":"archer","name":"Offline archer","guildName":"RANGE","nameColourArgb":-16711681,"x":301,"y":631,"direction":"Right",
                 "sprite":{"bodyLibrary":"CArmour/00","weaponLibrary":"ARWeapon/00",
                    "frameBaseOffset":0,"weaponFrameOffset":0,"directionStride":4,
                    "altBodyLibrary":"ARArmour/00","altWeaponLibrary":"ARWeapon/00 S",
                    "altFrameBaseOffset":0,"altWeaponFrameOffset":0}},
                {"objectId":"9005","kind":"player","classKey":"warrior","name":"Offline rider","guildName":"MOUNT","nameColourArgb":-23296,"x":304,"y":631,"direction":"Down",
                 "sprite":{"bodyLibrary":"CArmour/00","mountLibrary":"Mount/00",
                    "frameBaseOffset":0,"mountFrameOffset":0,"directionStride":4}}
                ,{"objectId":"9006","kind":"hero","classKey":"taoist","level":12,"name":"Owned hero","ownerName":"OFFLINE UI FIXTURE","nameColourArgb":-16711936,"x":300,"y":632,"direction":"Down",
                 "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4}}
            ],
            "groundDrops":[
                {"objectId":"9101","name":"Offline potion","nameColourArgb":-10040065,
                 "x":300,"y":635,"image":0,"quantity":1,"dropKind":"item"},
                {"objectId":"9102","name":"Gold","nameColourArgb":-256,
                 "x":304,"y":636,"image":114,"quantity":250,"dropKind":"gold"}
            ]
        })
        .to_string();
        if let Some(overlays) =
            crate::entity_overlays::project(&snapshot, scene.center_x, scene.center_y)
        {
            world
                .resource_mut::<crate::entity_overlays::ActorOverlayModel>()
                .replace(overlays);
        }
        if let Some(labels) =
            crate::ground_labels::project(&snapshot, scene.center_x, scene.center_y)
        {
            world
                .resource_mut::<crate::ground_labels::GroundDropLabelModel>()
                .replace(labels);
        }
        if let Some(pickups) = crate::ground_pickups::project(&snapshot) {
            world.insert_resource(pickups);
        }
        if !crate::live_entity::install_models(&snapshot, u64::MAX) {
            warn!("offline world-render preview entity model was not accepted");
        }
        if !crate::world_assets::request_packaged_map_atlas_load(scene, snapshot, u64::MAX) {
            warn!("offline world-render preview asset request was not accepted");
        }
    }
    populate_specimens(world, &scene);
    // Arm only after the new request is begun, the old panel closed and new
    // messages queued. A still-open old same-mode shop in frames0..2 cannot
    // satisfy this request's observation.
    world.resource_mut::<OfflineNpcPreviewReceipt>().scene =
        is_npc_service_preview(&scene).then(|| scene.clone());
    if scene == "inventory-amount" {
        world.resource_scope(|world, mut state: Mut<NativePlayerUiState>| {
            state.open_inventory_delete_for_slot(
                world.resource::<mir2_client_bevy::inventory::InventoryModel>(),
                11,
            );
        });
    }
    if scene == "mail-compose" {
        world
            .resource_mut::<NativePlayerUiState>()
            .core
            .mail_compose = Some(mir2_ui_core::state::MailComposeDraft::default());
    }
    info!("ANDROID_UI_PREVIEW_READY scene={scene}");
}

fn preview_panel_for_scene(scene: &str) -> UiPanel {
    match scene {
        "inventory" | "inventory-amount" => UiPanel::Inventory,
        "character" => UiPanel::Character,
        "skills" => UiPanel::Skill,
        "quests" | "quests-ingress" | "quest-confirmation" | "quest-alert" => UiPanel::QuestLog,
        "options" => UiPanel::Options,
        "platform" => UiPanel::PlatformSettings,
        "menu" => UiPanel::Menu,
        "gameshop" => UiPanel::GameShop,
        "mail" | "mail-compose" => UiPanel::Mail,
        "bigmap" => UiPanel::BigMap,
        "storage" | "storage-locked" => UiPanel::Storage,
        "group" => UiPanel::Group,
        "guild" => UiPanel::Guild,
        "trade" => UiPanel::Trade,
        "chat-settings" => UiPanel::ChatSettings,
        "npc" => UiPanel::NpcDialog,
        // The received model owns this dialog's visibility. A pinned manual
        // panel would keep shared world/HUD guards blocked after model Exit.
        // dialog.is_open still supplies the normal shared modal protection.
        "npc-ingress" => UiPanel::None,
        _ => UiPanel::None,
    }
}

fn initialize_panel(state: &mut mir2_ui_core::state::UiState, panel: UiPanel) {
    if panel == UiPanel::ChatSettings {
        // OpenChatSettings toggles the panel: start closed and let the shared
        // lifecycle create its draft instead of constructing a partial state.
        state.panel = UiPanel::None;
        *state =
            mir2_ui_core::reducer::reduce(state, mir2_ui_core::action::UiAction::OpenChatSettings)
                .state;
    } else {
        state.panel = panel;
    }
}

fn initialize_player_panel(state: &mut NativePlayerUiState, panel: UiPanel) {
    if panel == UiPanel::Skill {
        // Use the shared Skills/F11 entry, not only the core panel enum (which
        // otherwise leaves CharacterDialog on its default CHAR page).
        state.core.panel = UiPanel::None;
        state.toggle_skill();
    } else {
        initialize_panel(&mut state.core, panel);
    }
}

/// Server-shaped offline data exercises the same personal skill projection as
/// the network host. This is not an authenticated account or a learned skill.
fn offline_skill_ingress_model() -> Result<String, &'static str> {
    use serde_json::json;
    let mut ingress = crate::skill_ingress::AndroidSkillIngress::default();
    ingress.snapshot(
        &json!({"tick":100,"playerObjectId":42,
            "entities":[{"objectId":42,"kind":"selfPlayer","name":"OFFLINE SKILL FIXTURE"}],
        "knownSkills":[{"id":1,"spell":"FireBall","level":1,"castKind":"target","hotkey":1,
                "delayMs":2200,"cooldownRemainingMs":300,"cooldownRemainingTicks":1,"mpCost":7}]
        })
        .to_string(),
    )?;
    ingress.packet(
        &json!({"type":"packet","packet":"NewMagic","payload":{
            "hero":false,"magic":{"spell":"FireBall","name":"Offline FireBall","icon":27}
        }})
        .to_string(),
    )?;
    ingress.packet(
        &json!({"type":"packet","packet":"Magic","payload":{
            "spell":"FireBall","cast":false
        }})
        .to_string(),
    )?;
    ingress.packet(
        &json!({"type":"packet","packet":"MagicCast","payload":{
            "spell":"FireBall"
        }})
        .to_string(),
    )?;
    let mut model = None;
    ingress.flush(|raw| {
        model = Some(raw);
        true
    });
    model.ok_or("Offline skill model missing")
}

/// One server-shaped personal specimen for both inventory and player adapters.
/// These diagnostic values are not an account, item custody or gameplay result.
fn offline_personal_snapshot() -> String {
    use serde_json::json;
    let items: Vec<_> = (0..12)
        .map(|slot| {
            json!({"uniqueId":9000+slot,"key":format!("ui-only-{slot}"),
            "name":format!("UI specimen {slot}"),"quantity":slot+1,"slot":slot,"icon":100+slot,
            "description":"Offline visual specimen. Not a server-owned item."})
        })
        .collect();
    let stats: Vec<_> = [
        (0, 2),
        (1, 7),
        (2, 1),
        (3, 4),
        (4, 12),
        (5, 24),
        (6, 3),
        (7, 8),
        (8, 1),
        (9, 5),
        (10, 4),
        (11, 2),
        (12, 200),
        (13, 100),
        (14, 7),
        (15, 3),
        (16, 100),
        (17, 25),
        (18, 50),
        (21, 1),
        (22, 2),
        (23, 3),
        (30, 1),
        (31, 2),
        (32, 3),
        (33, 4),
        (34, 5),
        (35, 10),
        (36, 2),
    ]
    .into_iter()
    .map(|(stat, value)| json!({"stat":stat,"value":value}))
    .collect();
    json!({"playerObjectId":42,"mapFileName":"0","mapTitle":"OFFLINE Bichon fixture",
        "entities":[{"objectId":42,"kind":"selfPlayer","name":"OFFLINE UI FIXTURE",
            "x":300,"y":630,"direction":"Down","class":"Warrior","gender":"Male",
            "level":22,"hair":0,"wingEffect":0}],
        "playerHp":160,"playerMaxHp":200,"playerMp":60,"playerMaxMp":100,
        "playerExperience":125,"playerMaxExperience":1000,"playerCrystalStats":stats,
        "currentWeight":66,"maxWeight":100,"playerWeights":{"bag":66,"wear":25,"hand":9},
        "gold":12345,"credit":500,"inventoryCapacity":46,"inventoryItems":items,
        "equipmentItems":[
            {"uniqueId":9100,"key":"ui-equipped-0","name":"Offline source frame30",
             "quantity":1,"slot":"Weapon","icon":100,"stateImage":30},
            {"uniqueId":9101,"key":"ui-equipped-1","name":"Offline equipment slot1",
             "quantity":1,"slot":"Armour","icon":101}]})
    .to_string()
}

/// The production personal inventory adapter, with server-shaped offline data.
/// Android resolves geometry from the packaged originals; no live item custody.
fn offline_inventory_ingress_model() -> Result<String, &'static str> {
    let raw = offline_personal_snapshot();
    let mut ingress = crate::inventory_ingress::AndroidInventoryIngress::default();
    ingress.snapshot(&raw)?;
    let mut model = None;
    ingress.flush(
        |raw| {
            model = Some(raw);
            true
        },
        |_| true,
    );
    model.ok_or("Offline inventory model missing")
}

/// Exercises the same validated snapshot and public received-packet projection
/// as the native host. This feature-only specimen never connects to a Gateway.
fn offline_player_ingress_model() -> Result<(String, String), &'static str> {
    let raw = offline_personal_snapshot();
    crate::world_projection::project(&raw, "0", "OFFLINE UI FIXTURE", 300, 630)
        .ok_or("Invalid offline personal snapshot")?;
    let mut ingress = crate::player_ingress::AndroidPlayerIngress::default();
    ingress.snapshot(&raw)?;
    for (packet, payload) in [
        (
            "UserInformation",
            serde_json::json!({"objectId":42,"name":"OFFLINE UI FIXTURE","hp":80,"mp":20}),
        ),
        ("GainedGold", serde_json::json!({"amount":10})),
        ("LoseGold", serde_json::json!({"amount":3})),
        ("GainedCredit", serde_json::json!({"amount":5})),
        ("LoseCredit", serde_json::json!({"amount":2})),
    ] {
        if !ingress.packet(
            &serde_json::json!({"type":"packet","packet":packet,"payload":payload}).to_string(),
        )? {
            return Err("Offline player packet not accepted");
        }
    }
    let mut ui = None;
    let mut wallet = None;
    ingress.flush(
        |raw| {
            ui = Some(raw);
            true
        },
        |raw| {
            wallet = Some(raw);
            true
        },
    );
    Ok((
        ui.ok_or("Offline player UI missing")?,
        wallet.ok_or("Offline player wallet missing")?,
    ))
}

fn offline_received_chat_lines() -> Result<Vec<String>, &'static str> {
    let mut ingress = crate::chat_ingress::AndroidChatIngress::default();
    ingress.bind(42, "OFFLINE UI FIXTURE")?;
    for (packet, payload) in [
        (
            "Chat",
            serde_json::json!({"message":"OFFLINE system chat", "chatType":"System"}),
        ),
        (
            "ObjectChat",
            serde_json::json!({"objectId":99,"text":"OFFLINE neighbor: hello", "chatType":"Normal"}),
        ),
    ] {
        ingress.packet(
            &serde_json::json!({"type":"packet","packet":packet,"payload":payload}).to_string(),
        )?;
    }
    let mut lines = vec![];
    ingress.flush(|raw| {
        lines.push(raw);
        true
    });
    Ok(lines)
}

fn is_npc_service_preview(scene: &str) -> bool {
    matches!(
        scene,
        "npcshop" | "npcshop-sell" | "npcshop-repair" | "npcshop-srepair"
    )
}

fn begin_offline_npc_preview_request(state: &mut NativePlayerUiState) {
    // Only this feature-isolated fixture simulates an accepted request. It is
    // not an outbound Gateway command, authenticated NPC interaction or ACK.
    state.core.panel = UiPanel::None;
    state.begin_npc_service_request();
}

#[derive(Debug, PartialEq, Eq)]
enum OfflineNpcMessage {
    Catalogue(String),
    Service(String),
}

/// Exercise the actual Android scene-bound NPC adapter; do not hand-fill a
/// ShopModel or force a service panel open. All inputs here remain synthetic,
/// feature-isolated and marked OFFLINE. No transaction receipt is invented.
fn offline_npc_ingress_messages(scene: &str) -> Result<Vec<OfflineNpcMessage>, &'static str> {
    let (packet, payload) = match scene {
        "npcshop" => (
            "NPCGoods",
            serde_json::json!({"ownerObjectId":42,"mapFileName":"0","hideAddedStats":true,
                "list":[{"uniqueId":"9007199254740993","name":"OFFLINE received item",
                    "icon":7,"price":50,"stock":20,"count":1,"panelType":0}]}),
        ),
        "npcshop-sell" => ("NPCSell", serde_json::Value::Null),
        "npcshop-repair" => ("NPCRepair", serde_json::json!({"rate":1.25})),
        "npcshop-srepair" => ("NPCSRepair", serde_json::json!({"rate":2.0})),
        _ => return Err("Not an offline NPC service scene"),
    };
    let snapshot = offline_personal_snapshot();
    let mut player = crate::player_ingress::AndroidPlayerIngress::default();
    player.snapshot(&snapshot)?;
    let mut ingress = crate::npc_ingress::AndroidNpcIngress::default();
    ingress.snapshot(&snapshot, player.presentation_cursor())?;
    if !ingress.packet(
        &serde_json::json!({"type":"packet","packet":packet,"payload":payload}).to_string(),
        true,
        player.presentation_cursor(),
    )? {
        return Err("Offline NPC packet not accepted");
    }
    // Both callbacks append to the same FIFO so the catalogue cannot be
    // reordered behind its opening signal, even in this offline specimen.
    let messages = std::cell::RefCell::new(Vec::new());
    if !ingress.flush(
        true,
        |raw| {
            messages
                .borrow_mut()
                .push(OfflineNpcMessage::Catalogue(raw));
            true
        },
        |raw| {
            messages.borrow_mut().push(OfflineNpcMessage::Service(raw));
            true
        },
    ) {
        return Err("Offline NPC messages not drained");
    }
    Ok(messages.into_inner())
}

fn populate_specimens(world: &mut World, scene: &str) {
    if matches!(scene, "chat" | "chat-settings") {
        if let Ok(lines) = offline_received_chat_lines() {
            // Queue each server-shaped line once, never both prefill ChatModel
            // and enqueue (which would duplicate messages next frame).
            let mut queued = 0;
            for raw in lines {
                if mir2_bevy_runtime::native_ingest::push_native_chat_line(raw) {
                    queued += 1;
                }
            }
            info!(queued, "ANDROID_OFFLINE_RECEIVED_CHAT_QUEUED_NOT_LIVE");
        }
    }
    use mir2_client_bevy::{
        big_map::{BigMapInfo, BigMapModel, BigMapNpc, BigMapPoint, BigMapWorldIcon},
        game_shop::{GameShopEntry, GameShopModel},
        quest_model::{
            CombatTargetModel, CombatTargetUpdate, GroundPickupModel, Quest, QuestStatus,
            QuestTracker, RecentPickup,
        },
        shop::{NpcShopServiceMode, ShopGood, ShopModel},
        skill_model::{SkillEntry, SkillModel},
    };
    world.insert_resource(SkillModel {
        skills: (1..=6)
            .map(|id| SkillEntry {
                id,
                name: format!("UI skill {id}"),
                level: 1,
                key: None,
                cooldown_ms: 500,
                mp_cost: 5,
            })
            .collect(),
        ..default()
    });
    if scene == "skills" {
        match offline_skill_ingress_model()
            .map(mir2_bevy_runtime::native_ingest::push_native_skill_model)
        {
            Ok(true) => {
                info!("ANDROID_SKILL_INGRESS_OFFLINE queued owner42 FireBall castSequence1 remainingMs300");
            }
            _ => warn!("ANDROID_SKILL_INGRESS_OFFLINE rejected; not a live account"),
        }
    }
    world.insert_resource(QuestTracker {
        active_quests: vec![Quest {
            quest_index: 1,
            accept_npc_index: Some(99),
            finish_npc_index: Some(99),
            title: "Offline UI quest specimen".into(),
            npc_name: Some("UI NPC".into()),
            group: Some("UI fixtures".into()),
            min_level_needed: 1,
            detail: default(),
            status: QuestStatus::InProgress,
            objectives: vec![],
            rewards: vec![],
            unknown_text: None,
        }],
    });
    if matches!(scene, "quest-confirmation" | "quest-alert") {
        use mir2_client_bevy::quest_ui::QuestUiState;
        // This is a manual UI specimen, not a received packet/auth receipt.
        // Java preview rejects network and the shared controller owns decisions.
        world.init_resource::<QuestUiState>();
        let mut state = world.resource_mut::<QuestUiState>();
        state.select_quest(1);
        if scene == "quest-confirmation" {
            state.request_abandon_confirmation(1);
        } else {
            state.show_quest_alert(
                (0..30)
                    .map(|line| {
                        format!("OFFLINE phone message line {line}: no quest or reward granted.")
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        info!(scene, "ANDROID_QUEST_MODAL_MANUAL_UI_ONLY_NOT_LIVE");
    }
    if matches!(scene, "quests-ingress" | "npc-ingress") {
        use mir2_client_bevy::quest_model::{CompletedQuestTracker, NearbyNpcModel, NpcDialogModel};
        // Clear the manual legacy specimen before producing received models.
        // Failure must not silently fall back to quest1 or a fabricated dialog.
        world.insert_resource(QuestTracker::default());
        world.insert_resource(CompletedQuestTracker::default());
        world.insert_resource(NpcDialogModel::default());
        world.insert_resource(NearbyNpcModel::default());
        match crate::quest_preview::models(scene == "npc-ingress") {
            Ok(models) => {
                let count = models.tracker.active_quests.len();
                let quest_id = models.tracker.active_quests.first().map(|quest| quest.quest_index);
                let dialog_open = models.dialog.is_open;
                models.apply(world);
                info!(scene, count, ?quest_id, dialog_open,
                    "ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE");
            }
            Err(error) => warn!(error, "ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_FAILED_NOT_LIVE"),
        }
    }
    if scene == "multitouch" {
        let mut target = CombatTargetModel::default();
        target.apply(CombatTargetUpdate {
            object_id: 731,
            name: "OFFLINE TARGET".into(),
            hp: 9,
            max_hp: 9,
            is_player: false,
        });
        world.insert_resource(target);
        let mut pickups = GroundPickupModel::default();
        pickups.upsert(RecentPickup {
            object_id: Some(44),
            key: "object:44".into(),
            label: "OFFLINE PICKUP".into(),
            amount: 2,
            from_npc: Some("OFFLINE TARGET".into()),
        });
        world.insert_resource(pickups);
    }
    if is_npc_service_preview(scene) {
        // The real runtime consumer, not this fixture, applies catalogue and
        // service messages and opens the shared NPC surface on the next frame.
        world.insert_resource(ShopModel::default());
        match offline_npc_ingress_messages(scene) {
            Ok(messages) => {
                let mut catalogues = 0;
                let mut services = 0;
                let mut rejected = 0;
                for message in messages {
                    match message {
                        OfflineNpcMessage::Catalogue(raw) => {
                            if mir2_bevy_runtime::native_ingest::push_native_shop_model(raw) {
                                catalogues += 1;
                            } else {
                                rejected += 1;
                            }
                        }
                        OfflineNpcMessage::Service(raw) => {
                            if mir2_bevy_runtime::native_ingest::push_native_npc_shop_service(raw) {
                                services += 1;
                            } else {
                                rejected += 1;
                            }
                        }
                    }
                }
                info!(
                    scene,
                    catalogues, services, rejected, "ANDROID_NPC_INGRESS_OFFLINE_NOT_LIVE"
                );
            }
            Err(reason) => warn!(
                scene,
                reason, "ANDROID_NPC_INGRESS_OFFLINE_REJECTED_NOT_LIVE"
            ),
        }
    } else {
        // Unrelated legacy UI specimens are unchanged by the NPC ingress gate.
        world.insert_resource(ShopModel {
            service_mode: NpcShopServiceMode::Buy,
            goods: (0..8)
                .map(|index| ShopGood {
                    unique_id: 5000 + index,
                    name: format!("UI goods {index}"),
                    price: 10,
                    stock: 20,
                    count: 1,
                    icon: 100 + index as u16,
                    ..default()
                })
                .collect(),
            ..default()
        });
    }
    world.insert_resource(GameShopModel {
        items: (0..12)
            .map(|index| GameShopEntry {
                item_index: 1_000 + index,
                game_shop_index: 2_000 + index,
                item_name: format!("Offline product {}", index + 1),
                image: 100 + index as u32,
                gold_price: 100 + index as u32 * 10,
                credit_price: 10 + index as u32,
                category: "UI fixture".into(),
                stock: 20,
                stock_level: 20 - index,
                can_buy_credit: true,
                can_buy_gold: true,
                ..default()
            })
            .collect(),
        selected_game_shop_index: Some(2_000),
        ..default()
    });
    let mut big_map = BigMapModel::default();
    big_map.set_current_map(1);
    big_map.set_player_location(Some(1), BigMapPoint { x: 330, y: 270 });
    big_map.apply_world_map_setup(
        true,
        vec![BigMapWorldIcon {
            image_index: 1,
            title: "Offline world fixture".into(),
            map_index: 1,
        }],
        3_000,
    );
    big_map.apply_new_map_info(
        1,
        BigMapInfo {
            title: "Offline Bichon map fixture".into(),
            width: 700,
            height: 700,
            big_map: 412,
            movements: Vec::new(),
            npcs: (0..22)
                .map(|index| BigMapNpc {
                    index,
                    file_name: "NPC/00".into(),
                    name: format!("Offline NPC {}", index + 1),
                    map_index: 1,
                    location: BigMapPoint {
                        x: 120 + index * 17,
                        y: 180 + index * 11,
                    },
                    image: 0,
                    rate: 0,
                    show_on_big_map: true,
                    big_map_icon: 0,
                    object_id: 10_000 + index as u32,
                    icon: 0,
                    can_teleport_to: index == 0,
                })
                .collect(),
        },
    );
    let _ = big_map.select_npc(10_000);
    if !is_npc_service_preview(scene) {
        world.insert_resource(big_map);
    }
    // NPC replies are queued against the existing scene. Replacing its map
    // epoch here makes the production UI correctly close those replies as
    // stale, before an actual service window can be shown.
    use mir2_client_bevy::{
        inventory::InventoryModel,
        mail::{MailMessage, MailModel},
        quest_model::{NpcDialogLine, NpcDialogModel, NpcDialogOption},
        social::SocialModel,
        storage::StorageModel,
    };
    let inventory = match offline_inventory_ingress_model().and_then(|raw| {
        serde_json::from_str::<InventoryModel>(&raw)
            .map(|model| (raw, model))
            .map_err(|_| "Invalid offline inventory model")
    }) {
        Ok((_raw, model)) => {
            #[cfg(target_os = "android")]
            if mir2_bevy_runtime::native_ingest::push_native_inventory_model(_raw) {
                info!(
                    "ANDROID_INVENTORY_INGRESS_OFFLINE queued owner42 bag12 equip2 sourceFrame30"
                );
            } else {
                warn!("offline inventory model queue is unavailable");
            }
            model
        }
        Err(error) => {
            warn!("offline inventory source model unavailable: {error}");
            InventoryModel::default()
        }
    };
    let storage_items = inventory
        .items
        .iter()
        .cloned()
        .map(|mut item| {
            item.container = 4;
            item
        })
        .collect();
    world.insert_resource(inventory);
    if matches!(scene, "inventory" | "character") {
        match offline_player_ingress_model().and_then(|(ui, wallet)| {
            serde_json::from_str::<UiReadModel>(&ui)
                .map(|model| (model, ui, wallet))
                .map_err(|_| "Invalid offline player model")
        }) {
            Ok((model, _ui, _wallet)) => {
                // The returned value is an absolute server-presentation value;
                // do not compute another wallet or item rule in the UI specimen.
                world.resource_mut::<InventoryModel>().gold = model.player.gold;
                world.insert_resource(model);
                #[cfg(target_os = "android")]
                if mir2_bevy_runtime::native_ingest::push_native_ui_read_model(_ui)
                    && mir2_bevy_runtime::native_ingest::push_native_wallet_patch(_wallet)
                {
                    info!("ANDROID_PLAYER_INGRESS_OFFLINE queued owner42 hp80 mp20 gold12352 credit503 weights66/25/9 xp12.5%; NOT LIVE GAMEPLAY");
                } else {
                    warn!("offline personal presentation queue unavailable");
                }
            }
            Err(error) => warn!("offline personal source model unavailable: {error}"),
        }
    }
    world.insert_resource(StorageModel {
        items: storage_items,
        size: 80,
        has_expanded: true,
        has_password: scene == "storage-locked",
        unlocked: scene != "storage-locked",
        ..default()
    });
    world.insert_resource(MailModel {
        mails: vec![MailMessage {
            id: 1,
            sender: "UI fixture".into(),
            subject: "Offline specimen".into(),
            body: "This is an offline UI layout sample, not delivered mail.".into(),
            gold: 100,
            ..default()
        }],
        selected_id: Some(1),
    });
    if scene == "npc" {
        world.insert_resource(NpcDialogModel {
            is_open: true,
            npc_object_id: Some(99),
            npc_name: Some("UI NPC specimen".into()),
            lines: vec![NpcDialogLine {
                text: "Offline NPC dialog: check wrapping, selectable links and close controls."
                    .into(),
            }],
            options: vec![NpcDialogOption {
                option_id: "ui-only".into(),
                label: "UI-only option (no server)".into(),
                enabled: false,
            }],
        });
    }
    let mut social = world.resource_mut::<SocialModel>();
    if scene == "group" {
        social.group.active = true;
        social.group.allow_invites = true;
        social.group.leader_name = Some("OFFLINE UI FIXTURE".into());
        social.group.members = (0..9)
            .map(|index| mir2_client_bevy::social::GroupMemberModel {
                name: if index == 0 {
                    "OFFLINE UI FIXTURE".into()
                } else {
                    format!("UI member {index}")
                },
                leader: index == 0,
                online: index != 8,
                level: Some(22u16.saturating_sub(index as u16)),
                class: Some((index % 3) as u8),
                hp: Some(140 - index as i32 * 7),
                max_hp: Some(160),
                map: Some(if index % 2 == 0 { "0" } else { "1" }.into()),
            })
            .collect();
        social.group.member_maps = social
            .group
            .members
            .iter()
            .map(|member| (member.name.clone(), member.map.clone().unwrap_or_default()))
            .collect();
    }
    if scene == "trade" {
        social.trade.state = "open".into();
        social.trade.partner = Some("UI partner".into());
        social.trade.open_revision = 1;
    }
    if scene == "guild" {
        social.guild.name = Some("OFFLINE UI GUILD".into());
        social.guild.notice = vec!["Offline notice specimen. No guild was created.".into()];
        social.guild.member_count = 1;
        social.guild.max_members = 50;
        // Offline permission specimen only; the preview host cannot send requests.
        social.guild.permissions = vec!["notice".into()];
        social.guild.members = vec![mir2_client_bevy::social::GuildMemberModel {
            name: "OFFLINE UI FIXTURE".into(),
            id: 1,
            online: true,
            rank_name: Some("Guild Master".into()),
            rank_index: Some(0),
            ..default()
        }];
        social.guild.ranks = vec![mir2_client_bevy::social::GuildRankModel {
            name: "Guild Master".into(),
            index: 0,
            options: 0,
            members: social.guild.members.clone(),
        }];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::shop::{NpcShopServiceMode, ShopModel};

    #[test]
    fn chat_settings_preview_opens_with_an_applicable_draft() {
        let mut state = mir2_ui_core::state::UiState::default();
        state.screen = UiScreen::InGame;
        initialize_panel(&mut state, UiPanel::ChatSettings);
        assert_eq!(state.panel, UiPanel::ChatSettings);
        assert!(state.chat_settings_draft.is_some());
        let applied = mir2_ui_core::reducer::reduce(
            &state,
            mir2_ui_core::action::UiAction::ApplyChatSettings,
        )
        .state;
        assert_eq!(applied.panel, UiPanel::None);
    }
    #[test]
    fn item_specimens_have_identity_for_shared_local_dialogs() {
        let mut world = World::new();
        world.init_resource::<mir2_client_bevy::social::SocialModel>();
        populate_specimens(&mut world, "inventory-amount");
        let inventory = world.resource::<mir2_client_bevy::inventory::InventoryModel>();
        assert_eq!(inventory.items[11].unique_id, Some(9011));
        let storage = world.resource::<mir2_client_bevy::storage::StorageModel>();
        assert!(storage.has_expanded);
        assert!(storage.items.iter().all(|item| item.container == 4));
        let mut state = NativePlayerUiState::default();
        assert!(state.open_inventory_delete_for_slot(inventory, 11));
    }

    #[test]
    fn inventory_specimen_uses_typed_owner_ingress_and_original_equipment_index() {
        let raw = offline_inventory_ingress_model().unwrap();
        let inventory: mir2_client_bevy::inventory::InventoryModel =
            serde_json::from_str(&raw).unwrap();
        assert_eq!(inventory.items.len(), 14);
        assert_eq!(inventory.capacity, 46);
        assert_eq!(inventory.gold, 12345);
        let sword = &inventory.items[12];
        assert_eq!(
            (
                sword.unique_id,
                sword.container,
                sword.slot,
                sword.state_image
            ),
            (Some(9100), 2, 0, 30)
        );
        assert!(inventory.items[..12]
            .iter()
            .enumerate()
            .all(|(slot, item)| item.unique_id == Some(9000 + slot as u64)
                && item.quantity == slot as u32 + 1));
        // CPU tests have no APK AssetManager; never fabricate source geometry.
        assert_eq!(sword.state_image_width, 0);
    }
    #[test]
    fn character_specimen_uses_validated_shared_stats_and_absolute_wallet() {
        let (ui, wallet) = offline_player_ingress_model().unwrap();
        let model: UiReadModel = serde_json::from_str(&ui).unwrap();
        let player = &model.player;
        assert_eq!(
            (player.hp, player.max_hp, player.mp, player.max_mp),
            (80, 200, 20, 100)
        );
        assert_eq!((player.gold, player.credit), (12352, 503));
        assert_eq!(player.experience_percent_label(), "12.5%");
        assert_eq!(player.available_weight(), 34);
        assert!(player.current_weight_known);
        let weights = player.weights.unwrap();
        assert_eq!((weights.bag, weights.wear, weights.hand), (66, 25, 9));
        let stats = player.crystal_stats.as_ref().unwrap();
        assert_eq!(stats.len(), 29);
        for (id, value) in [(12, 200), (13, 100), (16, 100), (18, 50), (17, 25)] {
            assert_eq!(
                stats.iter().find(|stat| stat.stat == id).unwrap().value,
                value
            );
        }
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&wallet).unwrap(),
            serde_json::json!({"gold":12352,"credit":503})
        );
    }
    #[test]
    fn player_specimen_is_confined_to_character_and_bag_preview_scenes() {
        for scene in ["character", "inventory", "hud"] {
            let mut world = World::new();
            world.init_resource::<mir2_client_bevy::social::SocialModel>();
            world.init_resource::<UiReadModel>();
            populate_specimens(&mut world, scene);
            let player = &world.resource::<UiReadModel>().player;
            if scene == "hud" {
                assert!(player.crystal_stats.is_none());
                assert_eq!(player.gold, 0);
            } else {
                assert!(player.crystal_stats.is_some());
                assert_eq!(player.gold, 12352);
                assert_eq!(
                    world
                        .resource::<mir2_client_bevy::inventory::InventoryModel>()
                        .gold,
                    12352
                );
            }
        }
    }
    #[test]
    fn locked_storage_scene_exposes_only_a_secure_local_draft() {
        let mut world = World::new();
        world.init_resource::<mir2_client_bevy::social::SocialModel>();
        populate_specimens(&mut world, "storage-locked");
        let storage = world.resource::<mir2_client_bevy::storage::StorageModel>();
        assert!(storage.has_password);
        assert!(!storage.unlocked);
        assert!(storage.password_draft.is_empty());
    }
    #[test]
    fn game_shop_and_big_map_scenes_have_navigable_offline_fixtures() {
        let mut world = World::new();
        world.init_resource::<mir2_client_bevy::social::SocialModel>();
        populate_specimens(&mut world, "gameshop");
        let game_shop = world.resource::<mir2_client_bevy::game_shop::GameShopModel>();
        assert_eq!(game_shop.items.len(), 12);
        assert_eq!(game_shop.selected_game_shop_index, Some(2_000));
        assert!(game_shop.selected().is_some());

        let big_map = world.resource::<mir2_client_bevy::big_map::BigMapModel>();
        let rendered = big_map.render_snapshot();
        assert_eq!(
            rendered.map_image_url.as_deref(),
            Some("original-ui/MMap/412.png")
        );
        assert_eq!(rendered.npcs.len(), 18);
        assert!(big_map.selected_teleport_intent().is_some());
        assert!(big_map.world.enabled);
    }
    #[test]
    fn skill_preview_uses_typed_owner_ingress_not_an_unbound_placeholder() {
        let raw = offline_skill_ingress_model().unwrap();
        let skills: mir2_client_bevy::skill_model::SkillModel = serde_json::from_str(&raw).unwrap();
        assert_eq!(skills.authority.player_object_id, 42);
        assert_ne!(skills.authority.session_epoch, 0);
        assert_eq!(
            skills.selection_for_shortcut(1).unwrap().spell.as_deref(),
            Some("FireBall")
        );
        assert_eq!(skills.binding_for(1).cast_sequence, 1);
        assert_eq!(skills.binding_for(1).cooldown_remaining_ms, Some(300));
        assert_eq!(skills.binding_for(1).mp_cost, Some(7));
        assert!(skills.skill_key_ack.is_none());
    }

    #[test]
    fn skills_specimen_opens_shared_spell_page_directly() {
        let mut state = NativePlayerUiState::default();
        state.core.screen = UiScreen::InGame;
        initialize_player_panel(&mut state, UiPanel::Skill);
        assert!(state.equipment_open());
        assert_eq!(
            state.character_page,
            mir2_client_bevy::crystal_ui::overlays::CharacterPage::Spells
        );
    }

    #[test]
    fn received_chat_specimen_uses_public_packet_adapter_and_shared_model() {
        let lines = offline_received_chat_lines().unwrap();
        let mut model = mir2_client_bevy::chat::ChatModel::default();
        for raw in lines {
            model.push(serde_json::from_str(&raw).unwrap());
        }
        assert_eq!(model.lines.len(), 2);
        assert_eq!(model.lines[0].text, "OFFLINE system chat");
        assert_eq!(model.lines[0].channel, "System");
        assert_eq!(model.lines[1].text, "OFFLINE neighbor: hello");
        assert_eq!(model.lines[1].channel, "Normal");
    }

    #[test]
    fn scene_inventory_is_unique_and_bounded() {
        let set: std::collections::BTreeSet<_> = SCENES.iter().collect();
        assert_eq!(set.len(), SCENES.len());
        assert_eq!(SCENES.len(), 43);
    }

    #[test]
    fn npc_received_preview_releases_actual_shared_action_guard_after_dialog_exit() {
        let mut player = NativePlayerUiState::default();
        player.core.screen = UiScreen::InGame;
        initialize_player_panel(&mut player, preview_panel_for_scene("npc-ingress"));
        assert!(player.blocks_world_action(true, false), "An open received NPC still blocks actions");
        assert!(!player.blocks_world_action(false, false),
            "After received dialog Exit, a hidden pinned NpcDialog panel must not keep the HUD/action guard blocked");
        assert_eq!(preview_panel_for_scene("npc"), UiPanel::NpcDialog, "Preserve the older manual specimen");
        assert_eq!(preview_panel_for_scene("quests-ingress"), UiPanel::QuestLog);
    }

    #[test]
    fn quest_received_preview_uses_production_model_without_authorizing_main_host() {
        use mir2_client_bevy::quest_model::{CompletedQuestTracker, NearbyNpcModel, QuestTracker};
        for scene in ["quests-ingress", "npc-ingress"] {
            let mut world = World::new();
            world.init_resource::<mir2_client_bevy::social::SocialModel>();
            let mut host = crate::shared_shell::HostState::default();
            host.phase = "DISCONNECTED".into();
            world.insert_resource(host);
            world.insert_resource(NativeShellModel {screen:Screen::Login,..default()});
            world.insert_resource(crate::AndroidGatewayTransportEnabled(false));
            populate_specimens(&mut world, scene);
            let tracker = world.resource::<QuestTracker>();
            assert_eq!(tracker.active_quests[0].quest_index, 2100004,
                "Must use received metadata, not the manual quest1 placeholder");
            assert_eq!(tracker.active_quests[0].title, "OFFLINE RECEIVED DAILY QUEST");
            assert!(world.resource::<CompletedQuestTracker>().contains(9));
            assert_eq!(world.resource::<NearbyNpcModel>().npcs[0].object_id, 24);
            assert_eq!(world.resource::<crate::shared_shell::HostState>().phase, "DISCONNECTED");
            assert_eq!(world.resource::<NativeShellModel>().screen, Screen::Login);
            assert!(!world.resource::<crate::AndroidGatewayTransportEnabled>().0);
        }
    }

    #[test]
    fn modal_specimens_only_change_offline_view_state_not_main_host_authorization() {
        use mir2_client_bevy::quest_ui::QuestUiState;
        for scene in ["quest-confirmation", "quest-alert"] {
            let mut world = World::new();
            world.init_resource::<mir2_client_bevy::social::SocialModel>();
            let mut host = crate::shared_shell::HostState::default();
            host.phase = "DISCONNECTED".into();
            world.insert_resource(host);
            world.insert_resource(NativeShellModel {
                screen: Screen::Login,
                ..default()
            });
            world.insert_resource(crate::AndroidGatewayTransportEnabled(false));
            populate_specimens(&mut world, scene);
            assert!(world.resource::<QuestUiState>().blocks_world_input());
            assert_eq!(
                world.resource::<crate::shared_shell::HostState>().phase,
                "DISCONNECTED"
            );
            assert_eq!(world.resource::<NativeShellModel>().screen, Screen::Login);
            assert!(!world.resource::<crate::AndroidGatewayTransportEnabled>().0);
            assert_eq!(preview_panel_for_scene(scene), UiPanel::QuestLog);
        }
    }

    #[test]
    fn multitouch_scene_exposes_exact_offline_action_targets() {
        let mut world = World::new();
        world.init_resource::<mir2_client_bevy::social::SocialModel>();
        populate_specimens(&mut world, "multitouch");
        let target = world.resource::<mir2_client_bevy::quest_model::CombatTargetModel>();
        assert_eq!(
            target.target.as_ref().map(|target| target.object_id),
            Some(731)
        );
        let pickups = world.resource::<mir2_client_bevy::quest_model::GroundPickupModel>();
        assert_eq!(
            pickups.recent.front().and_then(|pickup| pickup.object_id),
            Some(44)
        );
    }

    #[test]
    fn npc_preview_waits_for_received_messages_without_inventing_shop_state() {
        for scene in [
            "npcshop",
            "npcshop-sell",
            "npcshop-repair",
            "npcshop-srepair",
        ] {
            let mut world = World::new();
            world.init_resource::<mir2_client_bevy::social::SocialModel>();
            populate_specimens(&mut world, scene);
            let shop = world.resource::<ShopModel>();
            assert!(
                shop.goods.is_empty(),
                "{scene} must wait for received catalogue"
            );
            assert_eq!(shop.service_mode, NpcShopServiceMode::Closed, "{scene}");
            assert!(!shop.supports_buy && !shop.supports_sell, "{scene}");
            assert!(shop.repair_rate.is_none(), "{scene}");
        }
    }

    #[test]
    fn npc_service_preview_scenes_expose_each_authoritative_service_mode() {
        for (scene, expected) in [
            ("npcshop-sell", NpcShopServiceMode::Sell),
            ("npcshop-repair", NpcShopServiceMode::Repair),
            ("npcshop-srepair", NpcShopServiceMode::SpecialRepair),
        ] {
            let messages = offline_npc_ingress_messages(scene).unwrap();
            assert_eq!(messages.len(), 1, "no invented Buy catalogue for {scene}");
            let OfflineNpcMessage::Service(raw) = &messages[0] else {
                panic!("expected only a service signal for {scene}");
            };
            let signal = serde_json::from_str(raw).unwrap();
            let mut shop = ShopModel::default();
            assert!(shop.apply_service_signal(signal));
            assert_eq!(shop.service_mode, expected);
            assert_eq!(
                shop.repair_rate.is_some(),
                expected != NpcShopServiceMode::Sell
            );
            assert!(shop.goods.is_empty());
            assert!(!shop.supports_buy);
            assert_eq!(shop.supports_sell, expected == NpcShopServiceMode::Sell);
            assert_eq!(
                shop.repair_rate,
                match expected {
                    NpcShopServiceMode::Repair => Some(1.25),
                    NpcShopServiceMode::SpecialRepair => Some(2.0),
                    _ => None,
                }
            );
        }
    }

    #[test]
    fn npc_preview_receipt_cannot_report_an_old_same_mode_window_as_new() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<mir2_client_bevy::social::SocialModel>();
        world.init_resource::<mir2_client_bevy::big_map::BigMapModel>();
        world.init_resource::<UiReadModel>();
        world.init_resource::<OfflineNpcPreviewReceipt>();
        world.insert_resource(PreviewRequest {
            scene: Some("npcshop".into()),
            remaining: 0,
        });
        let mut state = NativePlayerUiState::default();
        state.core.screen = UiScreen::InGame;
        state.core.panel = UiPanel::NpcShop;
        assert!(state.npc_shop_open());
        world.insert_resource(state);
        let mut shop = ShopModel::default();
        assert!(
            shop.apply_service_signal(mir2_client_bevy::shop::NpcShopServiceSignal {
                mode: NpcShopServiceMode::Buy,
                repair_rate: None,
            })
        );
        world.insert_resource(shop);
        for frame in 0..=3 {
            apply(&mut world);
            world.run_system_once(report_npc_preview_consumer).unwrap();
            assert!(
                !world.resource::<OfflineNpcPreviewReceipt>().logged,
                "frame {frame} cannot report the previous open shop as this request's reply"
            );
        }
        assert_eq!(
            world
                .resource::<OfflineNpcPreviewReceipt>()
                .scene
                .as_deref(),
            Some("npcshop")
        );
        // Only verify observer arming here. GPU/actual consumer completion is
        // separately observed by the emulator, not simulated by this test.
        world.resource_mut::<NativePlayerUiState>().core.panel = UiPanel::NpcShop;
        world.resource_mut::<ShopModel>().apply_service_signal(
            mir2_client_bevy::shop::NpcShopServiceSignal {
                mode: NpcShopServiceMode::Buy,
                repair_rate: None,
            },
        );
        world.run_system_once(report_npc_preview_consumer).unwrap();
        assert!(world.resource::<OfflineNpcPreviewReceipt>().logged);
    }

    #[test]
    fn npc_preview_retains_current_map_after_an_accepted_request() {
        for scene in [
            "npcshop",
            "npcshop-sell",
            "npcshop-repair",
            "npcshop-srepair",
        ] {
            let mut world = World::new();
            world.init_resource::<mir2_client_bevy::social::SocialModel>();
            let mut map = mir2_client_bevy::big_map::BigMapModel::default();
            map.set_current_map(0);
            map.set_player_location(
                Some(0),
                mir2_client_bevy::big_map::BigMapPoint { x: 300, y: 630 },
            );
            map.reset_epoch = 7;
            world.insert_resource(map.clone());
            populate_specimens(&mut world, scene);
            assert_eq!(
                world.resource::<mir2_client_bevy::big_map::BigMapModel>(),
                &map,
                "{scene} must not inject a map reset that closes its queued service reply"
            );
        }
    }

    #[test]
    fn npc_buy_preview_uses_exact_ingress_identity_currency_and_fifo() {
        let messages = offline_npc_ingress_messages("npcshop").unwrap();
        assert_eq!(messages.len(), 2);
        let OfflineNpcMessage::Catalogue(raw) = &messages[0] else {
            panic!("catalogue must precede service");
        };
        let mut shop: ShopModel = serde_json::from_str(raw).unwrap();
        assert_eq!(shop.service_mode, NpcShopServiceMode::Closed);
        assert_eq!(shop.goods.len(), 1);
        let good = &shop.goods[0];
        assert_eq!(good.unique_id, 9_007_199_254_740_993);
        assert_eq!(
            (good.icon, good.price, good.count, good.stock),
            (7, 50, 1, 20)
        );
        assert_eq!(good.name, "OFFLINE received item");
        assert!(!good.use_pearls);
        assert!(shop.hide_added_stats);
        let OfflineNpcMessage::Service(raw) = &messages[1] else {
            panic!("service must follow catalogue");
        };
        assert!(shop.apply_service_signal(serde_json::from_str(raw).unwrap()));
        assert_eq!(shop.service_mode, NpcShopServiceMode::Buy);
        assert!(shop.supports_buy && !shop.supports_sell);
    }

    #[test]
    fn npc_preview_request_does_not_force_open_the_panel_or_connect() {
        let mut state = NativePlayerUiState::default();
        state.core.panel = UiPanel::Inventory;
        state.request_npc_service_exit();
        begin_offline_npc_preview_request(&mut state);
        assert_eq!(state.core.panel, UiPanel::None);
        assert!(!state.npc_shop_open());
        assert!(state.accepts_npc_service_reply());
        assert!(offline_npc_ingress_messages("login").is_err());
    }

    #[test]
    fn social_preview_scenes_expose_bounded_offline_models() {
        let mut world = World::new();
        world.init_resource::<mir2_client_bevy::social::SocialModel>();

        populate_specimens(&mut world, "group");
        let social = world.resource::<mir2_client_bevy::social::SocialModel>();
        assert!(social.group.active);
        assert_eq!(
            social.group.leader_name.as_deref(),
            Some("OFFLINE UI FIXTURE")
        );
        assert_eq!(social.group.members.len(), 9);
        assert_eq!(social.group.member_maps.len(), 9);

        populate_specimens(&mut world, "guild");
        let social = world.resource::<mir2_client_bevy::social::SocialModel>();
        assert_eq!(social.guild.name.as_deref(), Some("OFFLINE UI GUILD"));
        assert_eq!(social.guild.members.len(), 1);
        assert_eq!(social.guild.ranks.len(), 1);

        populate_specimens(&mut world, "trade");
        let social = world.resource::<mir2_client_bevy::social::SocialModel>();
        assert_eq!(social.trade.state, "open");
        assert_eq!(social.trade.partner.as_deref(), Some("UI partner"));
        assert_eq!(social.trade.open_revision, 1);
    }
}
