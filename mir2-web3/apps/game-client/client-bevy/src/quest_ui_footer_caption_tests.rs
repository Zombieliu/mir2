//! Production diary footer trees and real CPU text layout, without a renderer/Gateway.
use super::*;
use crate::native_i18n::{self, Locale};
use bevy::asset::AssetApp;
use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::text::{ComputedTextBlock, TextLayoutInfo};

fn tracker() -> QuestTracker {
    QuestTracker {
        active_quests: (0..12)
            .map(|index| Quest {
                quest_index: 2_110_001 + index,
                accept_npc_index: None,
                finish_npc_index: None,
                title: "Opaque server {chapter} {0}".into(),
                npc_name: None,
                group: None,
                min_level_needed: 1,
                detail: default(),
                status: crate::quest_model::QuestStatus::InProgress,
                objectives: vec![],
                rewards: vec![],
                unknown_text: None,
            })
            .collect(),
    }
}

fn spawn_diary(world: &mut World, locale: Locale, page: usize, camera: Option<Entity>) {
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let tracker = tracker();
    let mut commands = Commands::new(&mut queue, world);
    let mut root = commands.spawn(Node {
        width: Val::Px(QUEST_DIARY_DESIGN_WIDTH),
        height: Val::Px(QUEST_DIARY_DESIGN_HEIGHT),
        ..default()
    });
    if let Some(camera) = camera {
        root.insert(UiTargetCamera(camera));
    }
    native_i18n::with_locale(locale, || {
        root.with_children(|parent| {
            render_guided_quest_diary_panel(
                parent,
                &tracker,
                &QuestGuidance::from_profile_name("newcomer-v2"),
                None,
                &QuestUiState {
                    diary_page: page,
                    ..default()
                },
                None,
                QuestRenderText::default(),
            );
        });
    });
    queue.apply(world);
}

fn footers(world: &mut World) -> Vec<(Entity, Entity)> {
    let buttons = world
        .query::<(Entity, &Node, &QuestUiButtonVisual)>()
        .iter(world)
        .filter(|(_, node, _)| {
            node.top == Val::Px(408.) && node.width == Val::Px(62.) && node.height == Val::Px(22.)
        })
        .map(|(entity, _, _)| entity)
        .collect::<Vec<_>>();
    assert_eq!(
        buttons.len(),
        2,
        "real diary retains exactly two narrow footers"
    );
    buttons
        .into_iter()
        .map(|button| {
            let children = world
                .query::<(Entity, &Text, &ChildOf)>()
                .iter(world)
                .filter(|(_, _, parent)| parent.parent() == button)
                .map(|(entity, _, _)| entity)
                .collect::<Vec<_>>();
            assert_eq!(children.len(), 1, "literal caption is painted once");
            (button, children[0])
        })
        .collect()
}

#[test]
fn actual_ten_locale_diary_footer_captions_keep_single_line_hitrects_and_disabled_actions() {
    for locale in Locale::ALL {
        for page in 0..2 {
            let mut world = World::new();
            spawn_diary(&mut world, locale, page, None);
            for (button, caption) in footers(&mut world) {
                let node = world.get::<Node>(button).unwrap();
                let previous = node.left == Val::Px(22.);
                assert!(previous || node.left == Val::Px(228.));
                assert_eq!(node.padding, UiRect::axes(Val::Px(3.), Val::Px(2.)));
                let enabled = if previous { page == 1 } else { page == 0 };
                assert_eq!(
                    world.get::<QuestUiButtonVisual>(button).unwrap().enabled,
                    enabled
                );
                assert_eq!(world.get::<Button>(button).is_some(), enabled);
                let expected_action = enabled.then_some(if previous {
                    QuestUiButton::GuidedDiaryPrevious
                } else {
                    QuestUiButton::GuidedDiaryNext
                });
                assert_eq!(world.get::<QuestUiButton>(button).cloned(), expected_action);
                let expected = native_i18n::for_locale(
                    locale,
                    if previous { "ui.previous" } else { "ui.next" },
                    "missing Native caption",
                );
                assert_eq!(world.get::<Text>(caption).unwrap().0, expected);
                let size = world.get::<TextFont>(caption).unwrap().font_size;
                let FontSize::Px(size) = size else {
                    panic!("authored pixel caption size")
                };
                assert!((8.0..=10.0).contains(&size), "{locale:?}: {size}");
                if previous && matches!(locale, Locale::Russian | Locale::Indonesian) {
                    assert!(size < 10.0, "actual long caption must shrink: {locale:?}");
                }
                assert_eq!(
                    world.get::<TextLayout>(caption).unwrap().linebreak,
                    LineBreak::NoWrap
                );
                assert_eq!(world.get::<FocusPolicy>(button), Some(&FocusPolicy::Block));
            }
        }
    }
}

fn shaped_diary(locale: Locale) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
        bevy::image::ImagePlugin::default(),
        bevy::transform::TransformPlugin,
        bevy::camera::visibility::VisibilityPlugin,
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ));
    app.init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<bevy::image::TextureAtlasLayout>()
        .init_asset::<bevy::mesh::Mesh>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.world_mut()
        .resource_mut::<bevy::text::FontCx>()
        .collection = fontique::Collection::new(fontique::CollectionOptions {
        system_fonts: false,
        ..default()
    });
    let file = match locale {
        Locale::SimplifiedChinese => "NotoSansCJKsc-Regular.otf",
        Locale::TraditionalChinese => "NotoSansTC.ttf",
        Locale::Hindi => "NotoSansDevanagari-Regular.ttf",
        Locale::Thai => "NotoSansThai-Regular.ttf",
        Locale::Arabic => "NotoSansArabic-Regular.ttf",
        _ => "NotoSans-Regular.ttf",
    };
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/public/original-ui/fonts")
            .join(file),
    )
    .expect("same bundled font used by the portable client");
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bytes));
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(1024, 768),
                        scale_factor: 1.,
                    }),
                    ..default()
                },
                viewport: Some(Viewport {
                    physical_size: UVec2::new(1024, 768),
                    ..default()
                }),
                ..default()
            },
        ))
        .id();
    spawn_diary(app.world_mut(), locale, 0, Some(camera));
    // The portable host supplies the same authored font to the actual painter
    // descendants. Keep its Text/Node/layout unchanged and use no system fallback.
    for mut face in app
        .world_mut()
        .query::<&mut TextFont>()
        .iter_mut(app.world_mut())
    {
        face.font = font.clone().into();
    }
    for _ in 0..5 {
        app.update();
    }
    app
}

#[test]
fn actual_ten_locale_footer_glyphs_fit_original_inner_rect_on_one_rasterized_line() {
    for locale in Locale::ALL {
        let mut app = shaped_diary(locale);
        for (button, caption) in footers(app.world_mut()) {
            let world = app.world();
            let literal = &world.get::<Text>(caption).unwrap().0;
            let layout = world.get::<TextLayoutInfo>(caption).unwrap();
            let computed = world.get::<ComputedTextBlock>(caption).unwrap();
            assert!(
                !layout.glyphs.is_empty(),
                "{locale:?}: actual caption must rasterize"
            );
            assert_eq!(
                computed.buffer().lines().count(),
                1,
                "{locale:?}: {literal}"
            );
            assert!(
                layout.glyphs.iter().all(|glyph| glyph.line_index == 0),
                "{locale:?}: {literal}"
            );
            let mut missing = 0;
            for line in computed.buffer().lines() {
                for run in line.runs() {
                    for cluster in run.clusters() {
                        missing += cluster.glyphs().filter(|glyph| glyph.id == 0).count();
                    }
                }
            }
            assert_eq!(
                missing, 0,
                "{locale:?}: full literal must have real glyphs: {literal}"
            );
            assert!(
                layout.size.x <= 56.01 && layout.size.y <= 18.01,
                "{locale:?}: {literal:?} measured {:?} outside original 62x22 minus 6x4 padding",
                layout.size
            );
            println!(
                "footer {} {literal:?}: {:?}, {:?}, {} glyphs, 1 line",
                locale.code(),
                world.get::<TextFont>(caption).unwrap().font_size,
                layout.size,
                layout.glyphs.len()
            );
            let node = world.get::<ComputedNode>(button).unwrap();
            assert_eq!(node.size(), Vec2::new(62., 22.));
        }
    }
}

#[test]
fn unrelated_long_npc_option_keeps_full_literal_original_size_and_wrap_policy() {
    let mut world = World::new();
    let literal = "Long opaque NPC {chapter} {0}: leave this option fully wrapped";
    let mut queue = bevy::ecs::world::CommandQueue::default();
    Commands::new(&mut queue, &world)
        .spawn(Node::default())
        .with_children(|parent| {
            quest_log_text_button_at_raw(
                parent,
                QuestLogRect::new(1., 2., 62., 44.),
                literal,
                QuestUiButton::SelectNpcDialog {
                    target: "@literal:unchanged".into(),
                },
                true,
            );
        });
    queue.apply(&mut world);
    let (text, face, layout) = world
        .query::<(&Text, &TextFont, &TextLayout)>()
        .single(&world)
        .unwrap();
    assert_eq!(text.0, literal);
    assert_eq!(face.font_size, FontSize::Px(10.));
    assert_eq!(layout.linebreak, LineBreak::WordOrCharacter);
    assert!(world.query::<&QuestUiButton>().iter(&world).any(|action|
        matches!(action, QuestUiButton::SelectNpcDialog { target } if target == "@literal:unchanged")));
}
