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
    "world-render",
    "inventory",
    "character",
    "skills",
    "quests",
    "options",
    "platform",
    "menu",
    "gameshop",
    "npcshop",
    "mail",
    "bigmap",
    "storage",
    "group",
    "guild",
    "trade",
    "chat-settings",
    "npc",
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

pub fn install(app: &mut App) {
    app.init_resource::<PreviewRequest>()
        .add_systems(Update, report_world_render_ready)
        .add_systems(Update, start_world_render_motion_specimen)
        .add_systems(Update, report_world_render_motion_pose)
        .add_systems(PostUpdate, apply);
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
    *started = true;
}

fn apply_offline_entity_packet(packet: &str) -> bool {
    match crate::live_entity::apply_packet(packet) {
        crate::live_entity::LiveEntityPacketOutcome::Applied { models, render, .. } => {
            let models_ready =
                mir2_bevy_runtime::native_ingest::push_native_entity_model_set(models);
            let render_ready = render
                .map(mir2_bevy_runtime::native_ingest::push_native_entity_render_state)
                .unwrap_or(false);
            models_ready && render_ready
        }
        crate::live_entity::LiveEntityPacketOutcome::Ignored
        | crate::live_entity::LiveEntityPacketOutcome::Rejected => false,
    }
}

fn report_world_render_motion_pose(
    poses: Res<mir2_bevy_runtime::PresentationPoseBuffer>,
    time: Res<Time>,
    _main_thread: NonSend<crate::shared_shell::AndroidPresentationMainThread>,
    mut saw_active: Local<bool>,
    mut saw_settled: Local<bool>,
    mut reported_diagnostics: Local<bool>,
) {
    if !*reported_diagnostics && time.elapsed().as_millis() >= 4_500 {
        let diagnostics = mir2_bevy_runtime::get_mir2_remote_motion_presentation_diagnostics();
        info!(%diagnostics, "ANDROID_REMOTE_BACKSTEP_DIAGNOSTICS");
        *reported_diagnostics = true;
    }
    let Some((x, y)) = poses.native_overlay_entity_offset("9003") else {
        return;
    };
    if !*saw_active && (x.abs() > f32::EPSILON || y.abs() > f32::EPSILON) {
        info!(
            offset_x = x,
            offset_y = y,
            "ANDROID_REMOTE_BACKSTEP_POSE_ACTIVE"
        );
        *saw_active = true;
    } else if *saw_active && !*saw_settled && x.abs() <= f32::EPSILON && y.abs() <= f32::EPSILON {
        info!("ANDROID_REMOTE_BACKSTEP_POSE_SETTLED");
        *saw_settled = true;
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
    let panel = match scene.as_str() {
        "inventory" | "inventory-amount" => UiPanel::Inventory,
        "character" => UiPanel::Character,
        "skills" => UiPanel::Skill,
        "quests" => UiPanel::QuestLog,
        "options" => UiPanel::Options,
        "platform" => UiPanel::PlatformSettings,
        "menu" => UiPanel::Menu,
        "gameshop" => UiPanel::GameShop,
        "npcshop" => UiPanel::NpcShop,
        "mail" | "mail-compose" => UiPanel::Mail,
        "bigmap" => UiPanel::BigMap,
        "storage" => UiPanel::Storage,
        "group" => UiPanel::Group,
        "guild" => UiPanel::Guild,
        "trade" => UiPanel::Trade,
        "chat-settings" => UiPanel::ChatSettings,
        "npc" => UiPanel::NpcDialog,
        _ => UiPanel::None,
    };
    let mut state = world.resource_mut::<NativePlayerUiState>();
    state.core.screen = UiScreen::InGame;
    initialize_panel(&mut state.core, panel);
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
    if matches!(scene.as_str(), "hud" | "world-render") {
        // Offline presentation specimen only, never a Gateway bootstrap.
        model.player.map_name = Some("BichonProvince".into());
    }
    drop(model);
    if matches!(scene.as_str(), "hud" | "world-render") {
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
                 "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4,
                    "altBodyLibrary":"ARArmour/00","altWeaponLibrary":"ARWeapon/00 S",
                    "altFrameBaseOffset":0,"altWeaponFrameOffset":0}},
                {"objectId":"9005","kind":"player","classKey":"warrior","name":"Offline rider","guildName":"MOUNT","nameColourArgb":-23296,"x":304,"y":631,"direction":"Down",
                 "sprite":{"bodyLibrary":"CArmour/00","mountLibrary":"Mount/00",
                    "frameBaseOffset":0,"mountFrameOffset":0,"directionStride":4}}
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

fn populate_specimens(world: &mut World, scene: &str) {
    use mir2_client_bevy::{
        quest_model::{Quest, QuestStatus, QuestTracker},
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
    world.insert_resource(ShopModel {
        service_mode: NpcShopServiceMode::Buy,
        supports_buy: true,
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
    use mir2_client_bevy::{
        inventory::{InventoryModel, ItemModel},
        mail::{MailMessage, MailModel},
        quest_model::{NpcDialogLine, NpcDialogModel, NpcDialogOption},
        social::SocialModel,
        storage::StorageModel,
    };
    let items: Vec<ItemModel> = (0..12)
        .map(|slot| {
            serde_json::from_value(serde_json::json!({
        "uniqueId":9000+slot, "key":format!("ui-only-{slot}"), "name":format!("UI specimen {slot}"),
        "quantity":slot+1, "slot":slot, "container":0, "icon":100+slot,
        "description":"Offline visual specimen. Not a server-owned item."
    })).expect("static UI item specimen")
        })
        .collect();
    world.insert_resource(InventoryModel {
        items: items.clone(),
        gold: 12345,
        ..default()
    });
    world.insert_resource(StorageModel {
        items,
        size: 80,
        unlocked: true,
        ..default()
    });
    world.insert_resource(MailModel {
        mails: vec![MailMessage {
            id: 1,
            sender: "UI fixture".into(),
            subject: "Offline specimen".into(),
            body: "This is an offline UI layout sample, not delivered mail.".into(),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut state = NativePlayerUiState::default();
        assert!(state.open_inventory_delete_for_slot(inventory, 11));
    }
    #[test]
    fn scene_inventory_is_unique_and_bounded() {
        let set: std::collections::BTreeSet<_> = SCENES.iter().collect();
        assert_eq!(set.len(), SCENES.len());
        assert_eq!(SCENES.len(), 34);
    }
}
