use super::*;
use mir2_client_bevy::crystal_ui::overlays::{
    CharacterPage, NativePlayerUiState, OverlayEquipment, OverlayInventory,
    CRYSTAL_CHARACTER_PANEL_RECT,
};
use mir2_ui_core::state::{UiPanel, UiScreen};

fn fitted_app(panel: UiPanel, density: f32) -> App {
    let mut app = App::new();
    let mut player = NativePlayerUiState::default();
    player.core.screen = UiScreen::InGame;
    player.core.panel = panel;
    app.insert_resource(HostState::default())
        .insert_resource(NativeShellModel {
            screen: Screen::InGame,
            ..default()
        })
        .insert_resource(player)
        .init_resource::<mir2_client_bevy::quest_model::NpcDialogModel>()
        .insert_resource(UiScale(1.0))
        .init_resource::<mir2_client_bevy::crystal_ui::hud::CrystalBeltPresentation>()
        .add_systems(Update, fit_stage);
    let mut window = Window::default();
    window.resolution = bevy::window::WindowResolution::new(
        (851.0 * density).round() as u32,
        (393.0 * density).round() as u32,
    );
    window.resolution.set_scale_factor_override(Some(density));
    app.world_mut().spawn(window);
    app
}

fn screen_rect(app: &mut App, entity: Entity) -> Rect {
    let window = app
        .world_mut()
        .query::<&Window>()
        .single(app.world())
        .unwrap();
    let fit = CrystalStageTransform::fit(window.width(), window.height());
    let node = app.world().get::<Node>(entity).unwrap();
    let (origin, size) = focus_panel_rect(node).unwrap();
    let transform = app.world().get::<UiTransform>(entity).unwrap();
    let (Val::Px(x), Val::Px(y)) = (transform.translation.x, transform.translation.y) else {
        panic!("expected an exact logical-pixel panel transform");
    };
    let center = origin + size * 0.5 + Vec2::new(x, y);
    let half = size * transform.scale * 0.5;
    let stage_origin = Vec2::new(fit.offset_x, fit.offset_y);
    Rect::from_corners(
        stage_origin + (center - half) * fit.scale,
        stage_origin + (center + half) * fit.scale,
    )
}

#[test]
fn android_character_and_all_four_shared_pages_are_focused_without_changing_authored_nodes() {
    for density in [1.0, 2.75, 3.5] {
        for page in [
            CharacterPage::Character,
            CharacterPage::Stats1,
            CharacterPage::Stats2,
            CharacterPage::Spells,
        ] {
            let mut app = fitted_app(UiPanel::Character, density);
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .character_page = page;
            let rect = CRYSTAL_CHARACTER_PANEL_RECT;
            let authored = Node {
                position_type: PositionType::Absolute,
                left: px(rect.left),
                top: px(rect.top),
                width: px(rect.width),
                height: px(rect.height),
                ..default()
            };
            let panel = app
                .world_mut()
                .spawn((OverlayEquipment, authored.clone(), UiTransform::default()))
                .id();
            app.update();
            assert_eq!(*app.world().get::<Node>(panel).unwrap(), authored);
            assert!(
                screen_rect(&mut app, panel).height() > 340.0,
                "{density} {page:?}: tiny character window retained"
            );
            assert_eq!(
                app.world().resource::<NativePlayerUiState>().character_page,
                page
            );
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .panel = UiPanel::None;
            app.update();
            assert_eq!(
                *app.world().get::<UiTransform>(panel).unwrap(),
                UiTransform::default()
            );
        }
    }
}

#[test]
fn android_inventory_focus_leaves_sidebar_and_thumb_surfaces_clear() {
    for density in [1.0, 2.75, 3.5] {
        let mut app = fitted_app(UiPanel::Inventory, density);
        let panel = app
            .world_mut()
            .spawn((
                OverlayInventory,
                UiTransform::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(20.0),
                    top: px(30.0),
                    width: px(316.0),
                    height: px(236.0),
                    ..default()
                },
            ))
            .id();
        app.update();
        let actual = screen_rect(&mut app, panel);
        let window = app
            .world_mut()
            .query::<&Window>()
            .single(app.world())
            .unwrap();
        let lane = crate::phone_panels::panel_sidebar(
            Vec2::new(window.width(), window.height()),
            Vec4::ZERO,
            0.0,
            true,
        )
        .unwrap();
        for chrome in [lane.status, lane.belt, lane.chat] {
            assert!(
                actual.intersect(chrome).is_empty(),
                "{density}: {actual:?} covers {chrome:?}"
            );
        }
        assert!(actual.min.x >= lane.workspace.min.x - 0.01);
        assert!(actual.max.x <= lane.workspace.max.x + 0.01);
        assert!(actual.min.y >= lane.workspace.min.y - 0.01);
        assert!(actual.max.y <= lane.workspace.max.y + 0.01);
        assert!(actual.height() > 320.0);
    }
}
