//! Headless ECS regressions: no WindowPlugin, renderer, GUI launch or input
//! injection. These are not native screenshots or human visual acceptance.

use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use crate::social::{GuildModel, GuildStorageItemModel, SocialModel, SocialPendingOperation};
use bevy::ecs::system::RunSystemOnce;

fn state() -> NativePlayerUiState {
    let mut state = NativePlayerUiState::default();
    state.core.panel = mir2_ui_core::state::UiPanel::Guild;
    state.guild_left_page = GuildLeftPage::Storage;
    state
}

fn guild() -> GuildModel {
    GuildModel {
        name: Some("SourceGuild".into()),
        my_rank_id: 0,
        my_options: 0x19,
        gold: 1_234_567,
        storage_items: vec![None; 112],
        ..default()
    }
}

fn fixture_image(width: u32, height: u32) -> Image {
    Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &[255, 255, 255, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

#[test]
fn guild_storage_ecs_has_source_grid_background_hit_rects_and_separate_arrow_art() {
    let mut app = tests::overlay_render_test_app();
    *app.world_mut().resource_mut::<NativePlayerUiState>() = state();
    app.world_mut().resource_mut::<SocialModel>().guild = guild();
    app.update();
    let world = app.world_mut();
    let mut cells = world.query::<(&OverlayGuildStorageCell, &Node)>();
    let mut slots = cells
        .iter(world)
        .map(|(cell, node)| {
            assert_eq!((node.width, node.height), (Val::Px(35.0), Val::Px(35.0)));
            assert_eq!(node.left, Val::Px(31.0 + (cell.slot % 8) as f32 * 36.0));
            assert_eq!(node.top, Val::Px(20.0 + (cell.slot / 8) as f32 * 36.0));
            assert_eq!(node.overflow, Overflow::DEFAULT);
            cell.slot
        })
        .collect::<Vec<_>>();
    slots.sort_unstable();
    assert_eq!(slots, (0..64).collect::<Vec<_>>());
    let mut images = world.query::<(&ImageNode, &Node)>();
    let background = images
        .iter(world)
        .find(|(image, _)| {
            image
                .image
                .path()
                .is_some_and(|path| path.to_string() == "original-ui/Prguse/1851.png")
        })
        .expect("original storage grid image")
        .1;
    assert_eq!(
        (
            background.left,
            background.top,
            background.width,
            background.height
        ),
        (Val::Px(30.0), Val::Px(19.0), Val::Px(292.0), Val::Px(308.0))
    );
    for (action, top, normal, hover, pressed) in [
        (OverlayButton::GuildStoragePreviousRow, 1.0, 197, 198, 199),
        (OverlayButton::GuildStorageNextRow, 318.0, 207, 208, 209),
    ] {
        let mut buttons = world.query::<(&OverlayButton, &Node, &CrystalImageButton)>();
        let (_, node, button) = buttons
            .iter(world)
            .find(|(candidate, _, _)| **candidate == action)
            .unwrap();
        assert_eq!(
            (node.left, node.top, node.width, node.height),
            (Val::Px(337.0), Val::Px(top), Val::Px(16.0), Val::Px(14.0))
        );
        assert_eq!(
            button.assets.normal,
            format!("original-ui/Prguse2/{normal}.png")
        );
        assert_eq!(
            button.assets.hover,
            format!("original-ui/Prguse2/{hover}.png")
        );
        assert_eq!(
            button.assets.pressed,
            format!("original-ui/Prguse2/{pressed}.png")
        );
        let mut images = world.query::<(&ImageNode, &Node)>();
        let art = images
            .iter(world)
            .find(|(image, _)| {
                image.image.path().is_some_and(|path| {
                    path.to_string() == format!("original-ui/Prguse2/{normal}.png")
                })
            })
            .unwrap()
            .1;
        assert_eq!(
            (art.left, art.top, art.width, art.height),
            (Val::Px(0.0), Val::Px(0.0), Val::Px(12.0), Val::Px(12.0))
        );
    }
    let mut labels = world.query::<(&Text, &Node)>();
    let gold = labels
        .iter(world)
        .find(|(text, _)| text.0 == "1,234,567")
        .unwrap()
        .1;
    assert_eq!(
        (gold.left, gold.top, gold.width, gold.height),
        (
            Val::Px(194.0),
            Val::Px(312.0),
            Val::Px(125.0),
            Val::Px(12.0)
        )
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .guild_storage
        .next_row();
    app.update();
    let world = app.world_mut();
    let mut cells = world.query::<(&OverlayGuildStorageCell, &Node)>();
    let mut slots = cells
        .iter(world)
        .map(|(cell, node)| {
            assert_eq!(node.top, Val::Px(20.0 + (cell.slot / 8 - 2) as f32 * 36.0));
            cell.slot
        })
        .collect::<Vec<_>>();
    slots.sort_unstable();
    assert_eq!(slots, (16..80).collect::<Vec<_>>());
}

#[test]
fn guild_storage_current_stack_image_and_count_do_not_use_stale_tooltip_count_or_real_info() {
    let mut app = tests::overlay_render_test_app();
    *app.world_mut().resource_mut::<NativePlayerUiState>() = state();
    let mut source = CrystalItemTooltipSourceModel {
        info: CrystalItemInfoModel {
            item_index: 710,
            name: "Amulet".into(),
            item_type: 8,
            shape: 0,
            stack_size: 300,
            image: 2960,
            ..default()
        },
        real_info: Some(CrystalItemInfoModel {
            item_type: 8,
            shape: 2,
            stack_size: 150,
            image: 123,
            ..default()
        }),
        user_item: Some(CrystalUserItemModel {
            unique_id: 101,
            count: 1,
            ..default()
        }),
        ..default()
    };
    // RealInfo changes the tooltip's semantic item, never the source icon.
    source.real_info.as_mut().unwrap().name = "Variant".into();
    let mut guild = guild();
    guild.storage_items[0] = Some(GuildStorageItemModel {
        unique_id: 101,
        item_index: 710,
        count: 300,
        tooltip_source: Some(source.clone()),
        ..default()
    });
    app.world_mut().resource_mut::<SocialModel>().guild = guild;
    app.update();
    let (icon, handle) = {
        let world = app.world_mut();
        let mut icons =
            world.query_filtered::<(Entity, &ImageNode, &Node), With<OriginalItemImage>>();
        let (entity, image, node) = icons.single(world).unwrap();
        assert_eq!(
            image.image.path().unwrap().to_string(),
            "original-ui/Items/3662.png"
        );
        assert_eq!(
            node.display,
            Display::None,
            "unloaded images must not use invented dimensions"
        );
        (entity, image.image.clone())
    };
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(handle.id(), fixture_image(32, 30))
        .unwrap();
    app.world_mut()
        .run_system_once(layout_original_item_images)
        .unwrap();
    let node = app.world().get::<Node>(icon).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(1.0), Val::Px(2.0), Val::Px(32.0), Val::Px(30.0))
    );
    let world = app.world_mut();
    let mut labels = world.query::<&Text>();
    assert!(labels.iter(world).any(|text| text.0 == "300"));
    assert_eq!(
        world.resource::<SocialModel>().guild.storage_items[0]
            .as_ref()
            .unwrap()
            .tooltip_source
            .as_ref(),
        Some(&source)
    );
    world.resource_mut::<SocialModel>().guild.storage_items[0]
        .as_mut()
        .unwrap()
        .count = 199;
    app.update();
    let world = app.world_mut();
    let mut icons = world.query_filtered::<&ImageNode, With<OriginalItemImage>>();
    assert_eq!(
        icons
            .single(world)
            .unwrap()
            .image
            .path()
            .unwrap()
            .to_string(),
        "original-ui/Items/3660.png"
    );
    let mut labels = world.query::<&Text>();
    assert!(labels.iter(world).any(|text| text.0 == "199"));
}

#[test]
fn guild_storage_original_images_keep_negative_odd_offsets_and_refresh_loaded_dimensions() {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .add_systems(Update, layout_original_item_images);
    for (cell_width, cell_height, width, height, left, top) in [
        (35, 35, 32, 30, 1, 2),
        (35, 35, 16, 28, 9, 3),
        (35, 35, 24, 27, 5, 4),
        (35, 35, 28, 29, 3, 3),
        (38, 34, 44, 30, -3, 2),
        (35, 35, 36, 38, 0, -1),
    ] {
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(fixture_image(width, height));
        let entity = app
            .world_mut()
            .spawn((
                OriginalItemImage {
                    cell_width,
                    cell_height,
                },
                ImageNode {
                    image: handle.clone(),
                    ..default()
                },
                Node::default(),
            ))
            .id();
        app.update();
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(
            (node.left, node.top, node.width, node.height),
            (
                Val::Px(left as f32),
                Val::Px(top as f32),
                Val::Px(width as f32),
                Val::Px(height as f32)
            )
        );
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .remove(handle.id());
        app.update();
        assert_eq!(
            app.world().get::<Node>(entity).unwrap().display,
            Display::None
        );
        app.world_mut().despawn(entity);
    }
}

#[test]
fn guild_storage_alpha_true_size_centres_without_cropping_and_refreshes_same_handle() {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .add_systems(Update, layout_original_item_images);
    let mut image = fixture_image(32, 30);
    let pixels = image.data.as_mut().unwrap();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = 0;
    }
    // The source returns size 28x24, not the bitmap's 32x30 and not origin (2,3).
    pixels[(3 * 32 + 2) * 4 + 3] = 1;
    pixels[(26 * 32 + 29) * 4 + 3] = 255;
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let entity = app
        .world_mut()
        .spawn((
            OriginalItemImage {
                cell_width: 35,
                cell_height: 35,
            },
            ImageNode {
                image: handle.clone(),
                ..default()
            },
            Node::default(),
        ))
        .id();
    app.update();
    let node = app.world().get::<Node>(entity).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(3.0), Val::Px(5.0), Val::Px(32.0), Val::Px(30.0))
    );
    // A content-only asset reload must invalidate cached bounds.
    {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let mut image = images.get_mut(handle.id()).unwrap();
        let pixels = image.data.as_mut().unwrap();
        pixels[3] = 255;
        pixels[(32 * 30 - 1) * 4 + 3] = 255;
    }
    app.update();
    let node = app.world().get::<Node>(entity).unwrap();
    assert_eq!((node.left, node.top), (Val::Px(1.0), Val::Px(2.0)));
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .get_mut(handle.id())
        .unwrap()
        .data = None;
    app.update();
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().display,
        Display::None
    );
}

#[test]
fn guild_storage_real_original_pngs_follow_get_true_size_and_keep_full_draw_dimensions() {
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/public/original-ui/Items");
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .add_systems(Update, layout_original_item_images);
    // Pinned MLibrary.GetTrueSize nonzero-alpha bounds, independently checked
    // against original Items.Lib, not metadata frame widths or synthetic art.
    for (index, width, height, true_width, true_height, cell_width, cell_height) in [
        (0, 32, 23, 31, 23, 35, 35),
        (116, 44, 30, 44, 30, 38, 34),
        (3660, 32, 30, 32, 30, 35, 35),
        (3661, 32, 30, 32, 30, 35, 35),
        (3662, 32, 30, 32, 30, 35, 35),
        (3673, 16, 28, 16, 28, 35, 35),
        (3674, 24, 27, 21, 27, 35, 35),
        (2960, 28, 29, 26, 29, 35, 35),
        (3675, 28, 29, 27, 29, 35, 35),
        (3670, 20, 29, 17, 29, 35, 35),
        (3671, 24, 27, 21, 27, 35, 35),
        (2961, 28, 29, 26, 29, 35, 35),
        (3672, 28, 29, 27, 29, 35, 35),
    ] {
        let png = std::fs::read(root.join(format!("{index}.png"))).unwrap();
        let image = Image::from_buffer(
            &png,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            bevy::asset::RenderAssetUsages::default(),
        )
        .unwrap();
        let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let entity = app
            .world_mut()
            .spawn((
                OriginalItemImage {
                    cell_width,
                    cell_height,
                },
                ImageNode {
                    image: handle,
                    ..default()
                },
                Node::default(),
            ))
            .id();
        app.update();
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.display, Display::Flex, "Items/{index}");
        assert_eq!(
            (node.left, node.top, node.width, node.height),
            (
                Val::Px(((cell_width - true_width) / 2) as f32),
                Val::Px(((cell_height - true_height) / 2) as f32),
                Val::Px(width as f32),
                Val::Px(height as f32)
            ),
            "Items/{index}"
        );
        app.world_mut().despawn(entity);
    }
}

#[test]
fn guild_storage_known_source_frame_zero_is_drawn_but_missing_source_is_not_guessed() {
    let mut app = tests::overlay_render_test_app();
    *app.world_mut().resource_mut::<NativePlayerUiState>() = state();
    let mut guild = guild();
    guild.storage_items[0] = Some(GuildStorageItemModel {
        unique_id: 101,
        item_index: 1,
        count: 1,
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_index: 1,
                name: "Source frame zero".into(),
                image: 0,
                ..default()
            },
            user_item: Some(CrystalUserItemModel {
                unique_id: 101,
                count: 1,
                ..default()
            }),
            ..default()
        }),
        ..default()
    });
    guild.storage_items[1] = Some(GuildStorageItemModel {
        unique_id: 102,
        item_index: 2,
        count: 1,
        ..default()
    });
    app.world_mut().resource_mut::<SocialModel>().guild = guild;
    app.update();
    let world = app.world_mut();
    let mut icons = world.query_filtered::<&ImageNode, With<OriginalItemImage>>();
    assert_eq!(
        icons
            .single(world)
            .unwrap()
            .image
            .path()
            .unwrap()
            .to_string(),
        "original-ui/Items/0.png"
    );
}

#[test]
fn guild_storage_source_tab_visibility_and_gold_withdraw_are_independent_rank_rules() {
    let mut app = tests::overlay_render_test_app();
    *app.world_mut().resource_mut::<NativePlayerUiState>() = state();
    for options in [0, 1, 8, 16, 24, 255] {
        for rank in [-1, 0, 1] {
            app.world_mut().resource_mut::<SocialModel>().guild = GuildModel {
                my_options: options,
                my_rank_id: rank,
                ..guild()
            };
            app.update();
            let world = app.world_mut();
            let mut query = world.query::<&OverlayButton>();
            let actions = query.iter(world).copied().collect::<Vec<_>>();
            assert_eq!(
                actions.contains(&OverlayButton::SelectGuildLeftPage(GuildLeftPage::Storage)),
                options & 0x18 != 0
            );
            assert_eq!(
                actions.contains(&OverlayButton::SelectGuildLeftPage(GuildLeftPage::Ranks)),
                options & 1 != 0
            );
            assert_eq!(
                actions.contains(&OverlayButton::GuildGoldWithdraw),
                rank == 0
            );
            assert!(actions.contains(&OverlayButton::GuildGoldDeposit));
        }
    }
}

#[test]
fn guild_gold_source_modal_has_exact_coin_input_button_geometry_and_invalid_ok_visibility() {
    let mut app = tests::overlay_render_test_app();
    let mut state = state();
    assert!(open_guild_gold_prompt(
        &mut state,
        &guild(),
        100,
        GuildGoldAction::Deposit,
        0
    ));
    *app.world_mut().resource_mut::<NativePlayerUiState>() = state;
    app.update();
    let world = app.world_mut();
    let mut inputs = world.query_filtered::<(&Node, &BorderColor), With<OverlayGuildGoldInput>>();
    let (input, border) = inputs.single(world).unwrap();
    assert_eq!(
        (input.left, input.top, input.width, input.height),
        (Val::Px(58.0), Val::Px(43.0), Val::Px(132.0), Val::Px(19.0))
    );
    assert_eq!(*border, BorderColor::all(Color::srgb(1.0, 0.647, 0.0)));
    let mut buttons = world.query::<(&OverlayButton, &Node)>();
    for (action, x, y, w, h) in [
        (OverlayButton::GuildGoldConfirm, 23.0, 76.0, 76.0, 25.0),
        (OverlayButton::GuildGoldCancel, 110.0, 76.0, 76.0, 25.0),
        (OverlayButton::GuildGoldClose, 180.0, 3.0, 24.0, 21.0),
    ] {
        let (_, node) = buttons
            .iter(world)
            .find(|(button, _)| **button == action)
            .unwrap();
        assert_eq!(
            (node.left, node.top, node.width, node.height),
            (Val::Px(x), Val::Px(y), Val::Px(w), Val::Px(h))
        );
    }
    let mut coins =
        world.query_filtered::<(&OriginalItemImage, &ImageNode), With<OriginalItemImage>>();
    let (cell, image) = coins.single(world).unwrap();
    assert_eq!((cell.cell_width, cell.cell_height), (38, 34));
    assert_eq!(
        image.image.path().unwrap().to_string(),
        "original-ui/Items/116.png"
    );
    world
        .resource_mut::<NativePlayerUiState>()
        .guild_gold_prompt
        .as_mut()
        .unwrap()
        .backspace();
    app.update();
    let world = app.world_mut();
    let mut actions = world.query::<&OverlayButton>();
    assert!(!actions
        .iter(world)
        .any(|action| *action == OverlayButton::GuildGoldConfirm));
    let mut inputs = world.query_filtered::<&BorderColor, With<OverlayGuildGoldInput>>();
    assert_eq!(
        *inputs.single(world).unwrap(),
        BorderColor::all(Color::srgb(1.0, 0.0, 0.0))
    );
}

#[test]
fn guild_gold_modal_submission_is_exact_nonoptimistic_deduplicated_and_throttled() {
    let mut state = state();
    let mut social = SocialModel {
        guild: guild(),
        ..default()
    };
    let original_guild = social.guild.clone();
    let mut intents = NativePlayerUiIntentQueue::default();
    assert!(open_guild_gold_prompt(
        &mut state,
        &social.guild,
        500,
        GuildGoldAction::Deposit,
        1000
    ));
    state.guild_gold_prompt.as_mut().unwrap().push_text("123");
    assert!(intents.intents.is_empty());
    assert!(confirm_guild_gold(
        &mut state,
        &mut social,
        500,
        1000,
        &mut intents
    ));
    assert_eq!(social.guild, original_guild);
    assert_eq!(
        intents.drain_intents(),
        vec![NativePlayerUiIntent::GuildStorageGoldChange {
            change_type: 0,
            amount: 123
        }]
    );
    assert_eq!(
        social.pending,
        vec![SocialPendingOperation::GuildStorageGold {
            change_type: 0,
            amount: 123
        }]
    );
    assert!(!confirm_guild_gold(
        &mut state,
        &mut social,
        500,
        1000,
        &mut intents
    ));
    assert!(!open_guild_gold_prompt(
        &mut state,
        &social.guild,
        500,
        GuildGoldAction::Deposit,
        1099
    ));
    assert!(open_guild_gold_prompt(
        &mut state,
        &social.guild,
        500,
        GuildGoldAction::Deposit,
        1100
    ));
    state.guild_gold_prompt.as_mut().unwrap().push_text("123");
    assert!(confirm_guild_gold(
        &mut state,
        &mut social,
        500,
        1100,
        &mut intents
    ));
    assert!(
        intents.intents.is_empty(),
        "matching in-flight request must not be resent"
    );
    assert_eq!(social.guild, original_guild);
}

#[test]
fn guild_gold_zero_and_stale_guild_rank_or_balance_do_not_send_mutations() {
    for action in [GuildGoldAction::Deposit, GuildGoldAction::Withdraw] {
        for invalid in ["zero", "guild", "balance", "rank"] {
            let mut state = state();
            let mut social = SocialModel {
                guild: guild(),
                ..default()
            };
            let mut intents = NativePlayerUiIntentQueue::default();
            assert!(open_guild_gold_prompt(
                &mut state,
                &social.guild,
                500,
                action,
                0
            ));
            state
                .guild_gold_prompt
                .as_mut()
                .unwrap()
                .push_text(if invalid == "zero" { "0" } else { "500" });
            let mut player_gold = 500;
            match invalid {
                "guild" => social.guild.name = Some("DifferentGuild".into()),
                "balance" => {
                    player_gold = 499;
                    social.guild.gold = 499;
                }
                "rank" => social.guild.my_rank_id = 1,
                _ => {}
            }
            let before = social.guild.clone();
            assert!(confirm_guild_gold(
                &mut state,
                &mut social,
                player_gold,
                0,
                &mut intents
            ));
            assert_eq!(social.guild, before);
            if invalid == "rank" && action == GuildGoldAction::Deposit {
                assert_eq!(
                    intents.intents.len(),
                    1,
                    "ordinary members may donate without item-store permission"
                );
            } else {
                assert!(intents.intents.is_empty(), "{action:?}/{invalid}");
            }
        }
    }
    let mut state = state();
    let mut not_leader = guild();
    not_leader.my_rank_id = 1;
    not_leader.permissions = vec!["CanRetrieveItem".into()];
    assert!(!open_guild_gold_prompt(
        &mut state,
        &not_leader,
        500,
        GuildGoldAction::Withdraw,
        0
    ));
}

#[test]
fn guild_gold_modal_blocks_underlying_buttons_even_in_the_frame_it_closes() {
    let mut app = tests::help_button_test_app();
    let mut state = state();
    assert!(open_guild_gold_prompt(
        &mut state,
        &guild(),
        500,
        GuildGoldAction::Deposit,
        0
    ));
    *app.world_mut().resource_mut::<NativePlayerUiState>() = state;
    app.world_mut()
        .spawn((Button, Interaction::Pressed, OverlayButton::GuildGoldCancel));
    app.world_mut()
        .spawn((Button, Interaction::Pressed, OverlayButton::CloseSocial));
    app.world_mut().spawn((
        Button,
        Interaction::Pressed,
        OverlayButton::GuildStorageNextRow,
    ));
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    assert!(state.guild_gold_prompt.is_none());
    assert!(state.guild_open());
    assert_eq!(state.guild_storage, GuildStorageUi::default());
    assert!(app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .intents
        .is_empty());
}

#[test]
fn guild_gold_real_keyboard_route_consumes_modal_keys_and_submits_only_on_enter() {
    let mut app = tests::help_keyboard_test_app();
    app.init_resource::<SocialModel>()
        .init_resource::<UiReadModel>();
    app.world_mut().resource_mut::<SocialModel>().guild = guild();
    app.world_mut().resource_mut::<UiReadModel>().player.gold = 500;
    let mut state = state();
    assert!(open_guild_gold_prompt(
        &mut state,
        &guild(),
        500,
        GuildGoldAction::Deposit,
        0
    ));
    state.guild_gold_prompt.as_mut().unwrap().push_text("321");
    *app.world_mut().resource_mut::<NativePlayerUiState>() = state;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyC);
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .equipment_open());
    assert!(app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .intents
        .is_empty());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .guild_gold_prompt
        .is_none());
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::GuildStorageGoldChange {
            change_type: 0,
            amount: 321
        }]
    );
    assert_eq!(app.world().resource::<UiReadModel>().player.gold, 500);
}

fn gold_keyboard_app() -> App {
    let mut app = tests::help_keyboard_test_app();
    app.init_resource::<SocialModel>()
        .init_resource::<UiReadModel>();
    app.world_mut().resource_mut::<SocialModel>().guild = guild();
    app.world_mut().resource_mut::<UiReadModel>().player.gold = 500;
    let mut ui = state();
    assert!(open_guild_gold_prompt(
        &mut ui,
        &guild(),
        500,
        GuildGoldAction::Deposit,
        0
    ));
    *app.world_mut().resource_mut::<NativePlayerUiState>() = ui;
    app
}

fn gold_key(app: &mut App, key_code: KeyCode, text: Option<&str>, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key: bevy::input::keyboard::Key::Character(text.unwrap_or("").into()),
        state,
        text: text.map(Into::into),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

#[test]
fn guild_gold_coalesced_digits_backspace_and_enter_submit_the_edited_not_old_amount() {
    for enter in [KeyCode::Enter, KeyCode::NumpadEnter] {
        let mut app = gold_keyboard_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(enter);
        for (key, text) in [
            (KeyCode::Digit1, Some("1")),
            (KeyCode::Digit2, Some("2")),
            (KeyCode::Backspace, None),
            (KeyCode::Digit3, Some("3")),
            (enter, Some("\r")),
            (KeyCode::Digit9, Some("9")), // Still owned by the closing modal.
        ] {
            gold_key(&mut app, key, text, ButtonState::Pressed);
        }
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::GuildStorageGoldChange {
                change_type: 0,
                amount: 13
            }]
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .guild_gold_prompt
            .is_none());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert!(open_guild_gold_prompt(
            &mut app.world_mut().resource_mut::<NativePlayerUiState>(),
            &guild(),
            500,
            GuildGoldAction::Deposit,
            100
        ));
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .guild_gold_prompt
                .as_ref()
                .unwrap()
                .draft,
            "500",
            "unread text must not leak into a later modal"
        );
    }
}

#[test]
fn guild_gold_escape_drains_its_keyboard_batch_without_leaking_to_the_next_modal() {
    let mut app = gold_keyboard_app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    gold_key(&mut app, KeyCode::Digit1, Some("1"), ButtonState::Pressed);
    gold_key(&mut app, KeyCode::Escape, None, ButtonState::Pressed);
    gold_key(&mut app, KeyCode::Digit9, Some("9"), ButtonState::Pressed);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .guild_gold_prompt
        .is_none());
    assert!(app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .intents
        .is_empty());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    assert!(open_guild_gold_prompt(
        &mut app.world_mut().resource_mut::<NativePlayerUiState>(),
        &guild(),
        500,
        GuildGoldAction::Deposit,
        0
    ));
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .guild_gold_prompt
            .as_ref()
            .unwrap()
            .draft,
        "500"
    );
}

#[test]
fn guild_gold_coalesced_control_a_release_then_digits_keeps_modifier_event_order() {
    let mut app = gold_keyboard_app();
    {
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        let prompt = ui.guild_gold_prompt.as_mut().unwrap();
        prompt.draft = "250".into();
        prompt.select_all = false;
    }
    // Ctrl is released in the final ButtonInput snapshot, but held at A.
    gold_key(&mut app, KeyCode::ControlLeft, None, ButtonState::Pressed);
    gold_key(&mut app, KeyCode::KeyA, Some("a"), ButtonState::Pressed);
    gold_key(&mut app, KeyCode::ControlLeft, None, ButtonState::Released);
    gold_key(&mut app, KeyCode::Digit4, Some("4"), ButtonState::Pressed);
    gold_key(&mut app, KeyCode::Digit2, Some("2"), ButtonState::Pressed);
    gold_key(&mut app, KeyCode::Enter, Some("\r"), ButtonState::Pressed);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::GuildStorageGoldChange {
            change_type: 0,
            amount: 42
        }]
    );
}

#[test]
fn guild_gold_enter_on_invalid_uint_closes_silently_like_source_invoke_click() {
    for draft in ["", "4294967296"] {
        let mut app = gold_keyboard_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .guild_gold_prompt
            .as_mut()
            .unwrap()
            .draft = draft.into();
        gold_key(&mut app, KeyCode::Enter, Some("\r"), ButtonState::Pressed);
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .guild_gold_prompt
            .is_none());
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0,
            "direct InvokeMouseClick does not play OnMouseClick's sound"
        );
    }
}

#[test]
fn guild_gold_mouse_buttons_keep_default_button_b_and_explicit_close_button_a() {
    use crate::audio::{NativeUiAudioQueue, NativeUiSound};
    for (action, sound, needs_prompt) in [
        (
            OverlayButton::GuildGoldDeposit,
            NativeUiSound::ButtonB,
            false,
        ),
        (
            OverlayButton::GuildGoldWithdraw,
            NativeUiSound::ButtonB,
            false,
        ),
        (
            OverlayButton::GuildGoldConfirm,
            NativeUiSound::ButtonB,
            true,
        ),
        (OverlayButton::GuildGoldCancel, NativeUiSound::ButtonB, true),
        (OverlayButton::GuildGoldClose, NativeUiSound::ButtonA, true),
    ] {
        let mut app = tests::help_button_test_app();
        app.init_resource::<UiReadModel>();
        app.world_mut().resource_mut::<SocialModel>().guild = guild();
        app.world_mut().resource_mut::<UiReadModel>().player.gold = 500;
        let mut ui = state();
        if needs_prompt {
            assert!(open_guild_gold_prompt(
                &mut ui,
                &guild(),
                500,
                GuildGoldAction::Deposit,
                0
            ));
        }
        *app.world_mut().resource_mut::<NativePlayerUiState>() = ui;
        tests::press_help_button(&mut app, action);
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![sound],
            "{action:?}"
        );
    }
}

fn pointer_app(scale: f32) -> (App, Entity) {
    let mut app = App::new();
    app.insert_resource(state())
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<MouseWheel>()
        .add_message::<CursorMoved>()
        .add_systems(Update, process_guild_storage_pointer);
    let mut window = Window::default();
    window.resolution.set(1024.0 * scale, 768.0 * scale);
    let id = app.world_mut().spawn((window, PrimaryWindow)).id();
    (app, id)
}

fn cursor(app: &mut App, window: Entity, x: f32, y: f32, scale: f32) {
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(
            (CRYSTAL_GUILD_PANEL_RECT.left + x) * scale,
            (CRYSTAL_GUILD_PANEL_RECT.top + 60.0 + y) * scale,
        )));
}

#[test]
fn guild_storage_wheel_is_window_cursor_modal_and_unit_scoped() {
    let (mut app, window) = pointer_app(2.0);
    cursor(&mut app, window, 40.0, 40.0, 2.0);
    app.world_mut().write_message(MouseWheel {
        phase: bevy::input::touch::TouchPhase::Moved,
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: -1.0,
        window,
    });
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .guild_storage
            .source_index(),
        2
    );
    for (unit, target, x, modal, focused) in [
        (MouseScrollUnit::Pixel, window, 40.0, false, true),
        (
            MouseScrollUnit::Line,
            Entity::PLACEHOLDER,
            40.0,
            false,
            true,
        ),
        (MouseScrollUnit::Line, window, 380.0, false, true),
        (MouseScrollUnit::Line, window, 40.0, true, true),
        (MouseScrollUnit::Line, window, 40.0, false, false),
    ] {
        cursor(&mut app, window, x, 40.0, 2.0);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = focused;
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .guild_gold_prompt =
            modal.then(|| GuildGoldPrompt::new(GuildGoldAction::Deposit, "SourceGuild".into(), 1));
        app.world_mut().write_message(MouseWheel {
            phase: bevy::input::touch::TouchPhase::Moved,
            unit,
            x: 0.0,
            y: -1.0,
            window: target,
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .guild_storage
                .source_index(),
            2
        );
    }
}

#[test]
fn guild_storage_thumb_drag_uses_source_grab_offset_scale_clamp_and_focus_release() {
    let (mut app, window) = pointer_app(2.0);
    cursor(&mut app, window, 340.0, 21.0, 2.0);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .guild_storage
        .is_dragging());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .guild_storage
            .source_index(),
        1,
        "a stationary press/hold must not invoke source OnMoving"
    );
    cursor(&mut app, window, 500.0, 257.0, 2.0); // Horizontal motion does not move the rail.
    app.update();
    let ui = app.world().resource::<NativePlayerUiState>().guild_storage;
    assert_eq!((ui.source_index(), ui.thumb_y()), (6, 252));
    cursor(&mut app, window, 500.0, 400.0, 2.0);
    app.update();
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .guild_storage
            .thumb_y(),
        298
    );
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .guild_storage
        .is_dragging());
}

#[test]
fn guild_storage_scroll_persists_on_hide_but_session_reset_clears_pointer_and_gold_modal() {
    let mut state = state();
    state.guild_storage.next_row();
    let before = state.guild_storage;
    state.close_windows();
    assert_eq!(state.guild_storage, before);
    state.core.panel = mir2_ui_core::state::UiPanel::Guild;
    state.guild_left_page = GuildLeftPage::Storage;
    assert!(open_guild_gold_prompt(
        &mut state,
        &guild(),
        500,
        GuildGoldAction::Deposit,
        0
    ));
    assert!(state.blocks_gameplay_keys());
    assert_eq!(
        modal_priority_for_state(&state, false, false),
        Some(OverlayModalPriority::SystemMenu)
    );
    state.reset_session();
    assert_eq!(state.guild_storage, GuildStorageUi::default());
    assert!(state.guild_gold_prompt.is_none());
    assert_eq!(state.guild_gold_ready_at_ms, 0);
}

#[test]
fn guild_storage_compressed_press_move_release_keeps_the_real_motion_before_ending_drag() {
    let (mut app, window) = pointer_app(1.0);
    {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
    }
    for (x, y) in [(340.0, 21.0), (500.0, 257.0)] {
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::new(
                CRYSTAL_GUILD_PANEL_RECT.left + x,
                CRYSTAL_GUILD_PANEL_RECT.top + 60.0 + y,
            ),
            delta: None,
        });
    }
    app.update();
    let ui = app.world().resource::<NativePlayerUiState>().guild_storage;
    assert_eq!((ui.source_index(), ui.thumb_y()), (6, 252));
    assert!(!ui.is_dragging());
}

#[test]
fn guild_gold_prompt_sync_cancels_changed_identity_revoked_rank_and_session_exit() {
    for reason in ["identity", "rank", "exit", "tab"] {
        let mut app = App::new();
        let mut state = state();
        assert!(open_guild_gold_prompt(
            &mut state,
            &guild(),
            500,
            GuildGoldAction::Withdraw,
            0
        ));
        app.insert_resource(state)
            .insert_resource(SocialModel {
                guild: guild(),
                ..default()
            })
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .add_systems(Update, sync_guild_storage_ui);
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .guild_gold_prompt
            .is_some());
        match reason {
            "identity" => {
                app.world_mut().resource_mut::<SocialModel>().guild.name =
                    Some("Replacement".into())
            }
            "rank" => {
                app.world_mut()
                    .resource_mut::<SocialModel>()
                    .guild
                    .my_rank_id = 1
            }
            "exit" => {
                app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login
            }
            "tab" => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .guild_left_page = GuildLeftPage::Notice
            }
            _ => unreachable!(),
        }
        app.update();
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .guild_gold_prompt
                .is_none(),
            "{reason}"
        );
    }
}
