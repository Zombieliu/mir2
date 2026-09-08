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
        .add_systems(PostUpdate, apply);
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
    state.core.panel = panel;
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
    drop(model);
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
        assert_eq!(SCENES.len(), 33);
    }
}
