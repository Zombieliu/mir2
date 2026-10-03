//! Actual shared chat-child regressions, not only the requested panel height.
use super::*;

struct ChatFixture {
    app: App,
    rows: Vec<Entity>,
    controls: Vec<(Entity, CrystalChatAction)>,
    input: Entity,
    root_origin: Vec2,
    scale: f32,
}

fn fixture(
    viewport: Vec2,
    density: f32,
    scale: f32,
    safe: Vec4,
    ime: f32,
    row_count: usize,
    focused: bool,
) -> ChatFixture {
    let mut app = App::new();
    let mut player = NativePlayerUiState::default();
    player.core.screen = mir2_ui_core::state::UiScreen::InGame;
    player.set_chat_focused(focused);
    let mut host = crate::shared_shell::HostState::default();
    host.ime_bottom = ime * density;
    app.insert_resource(UiScale(scale))
        .insert_resource(AndroidShellState {
            safe_area: crate::android_input::AndroidInsets {
                left: safe.x * density,
                top: safe.y * density,
                right: safe.z * density,
                bottom: safe.w * density,
            },
            ..default()
        })
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .insert_resource(player)
        .init_resource::<UiReadModel>()
        .init_resource::<CrystalChatState>()
        .init_resource::<CrystalBeltPresentation>()
        .insert_resource(host)
        .init_resource::<CrystalChatPointerBounds>()
        .add_systems(Startup, spawn)
        .add_systems(
            PostUpdate,
            (decorate_belt, fit_phone_hud, use_android_system_fonts).chain(),
        );
    let mut window = Window::default();
    window.resolution.set_scale_factor_override(Some(density));
    window.resolution.set(viewport.x, viewport.y);
    app.world_mut().spawn(window);
    // A nonzero desktop stage origin must be subtracted exactly once.
    let root_origin = Vec2::new(43.0, -27.0);
    let root = app
        .world_mut()
        .spawn((
            Node {
                left: px(root_origin.x),
                top: px(root_origin.y),
                ..default()
            },
            CrystalChatRoot,
        ))
        .id();
    let mut rows = Vec::new();
    for row in 0..row_count {
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                CrystalChatElement,
                CrystalChatLine { row },
                Text::new(format!("received row {row}")),
                crystal_text_font(12.0),
            ))
            .id();
        app.world_mut().entity_mut(root).add_child(entity);
        rows.push(entity);
    }
    let input = app
        .world_mut()
        .spawn((
            Node::default(),
            CrystalChatElement,
            CrystalChatInput,
            Text::new("unsent draft"),
            crystal_text_font(12.0),
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(input);
    let mut controls = Vec::new();
    for action in [
        CrystalChatAction::Up,
        CrystalChatAction::Down,
        CrystalChatAction::Settings,
        CrystalChatAction::FilterAll,
        CrystalChatAction::FilterShout,
        CrystalChatAction::FilterWhisper,
        CrystalChatAction::FilterLover,
        CrystalChatAction::FilterMentor,
        CrystalChatAction::FilterGroup,
        CrystalChatAction::FilterGuild,
        CrystalChatAction::TradeRequest,
        CrystalChatAction::Resize,
    ] {
        let entity = app
            .world_mut()
            .spawn((Node::default(), CrystalChatElement, action, Button))
            .id();
        app.world_mut().entity_mut(root).add_child(entity);
        controls.push((entity, action));
    }
    ChatFixture {
        app,
        rows,
        controls,
        input,
        root_origin,
        scale,
    }
}

impl ChatFixture {
    fn rect(&self, entity: Entity) -> Rect {
        let node = self.app.world().get::<Node>(entity).unwrap();
        let min = (Vec2::new(val_px(node.left), val_px(node.top)) + self.root_origin) * self.scale;
        Rect::from_corners(
            min,
            min + Vec2::new(val_px(node.width), val_px(node.height)) * self.scale,
        )
    }

    fn assert_rows(&self, first: usize) {
        for (index, entity) in self.rows.iter().enumerate() {
            assert_eq!(
                self.app.world().get::<Node>(*entity).unwrap().display,
                if index >= first {
                    Display::Flex
                } else {
                    Display::None
                },
                "received row {index}; expected visible suffix from {first}"
            );
            assert_eq!(
                self.app.world().get::<Text>(*entity).unwrap().0,
                format!("received row {index}")
            );
        }
    }

    fn assert_focused_bounds(&self, safe: Vec4, viewport: Vec2, ime: f32, density: f32) {
        let bounds = self
            .app
            .world()
            .resource::<CrystalChatPointerBounds>()
            .0
            .unwrap();
        let chat = Rect::from_corners(bounds.min / density, bounds.max / density);
        assert!(chat.min.x >= safe.x && chat.min.y >= safe.y);
        assert!(chat.max.x <= viewport.x - safe.z + 0.02);
        assert!(chat.max.y <= viewport.y - safe.w.max(ime) + 0.02);
        let input = self.rect(self.input);
        assert!(input.min.y >= safe.y && input.max.y <= chat.max.y + 0.02);
        for (entity, action) in &self.controls {
            assert_eq!(
                self.app.world().get::<Node>(*entity).unwrap().display,
                Display::Flex
            );
            assert_eq!(
                self.app.world().get::<CrystalChatAction>(*entity),
                Some(action)
            );
            let rect = self.rect(*entity);
            assert!(rect.min.y >= safe.y && rect.max.y <= chat.max.y + 0.02);
            assert!(
                input.intersect(rect).is_empty(),
                "draft overlaps {action:?}"
            );
            for row in &self.rows {
                if self.app.world().get::<Node>(*row).unwrap().display != Display::None {
                    let row_rect = self.rect(*row);
                    assert!(
                        row_rect.intersect(rect).is_empty(),
                        "row overlaps {action:?}"
                    );
                    assert!(row_rect.max.y <= rect.min.y + 0.02);
                }
            }
        }
    }
}

#[test]
fn focused_received_system_and_latest_peer_rows_both_remain_visible() {
    let viewport = Vec2::new(851.0, 393.0);
    let mut f = fixture(viewport, 1.0, 0.5, Vec4::ZERO, 170.0, 2, true);
    f.app.update();
    f.assert_rows(0);
    f.assert_focused_bounds(Vec4::ZERO, viewport, 170.0, 1.0);
}

#[test]
fn clamped_keyboard_height_keeps_latest_row_instead_of_oldest_requested_rows() {
    let viewport = Vec2::new(568.0, 262.0);
    let safe = Vec4::new(12.0, 8.0, 12.0, 12.0);
    let mut f = fixture(viewport, 1.0, 0.35, safe, 100.0, 11, true);
    f.app.update();
    f.assert_rows(10);
    f.assert_focused_bounds(safe, viewport, 100.0, 1.0);
}

#[test]
fn focused_history_suffix_is_stable_across_density_scale_and_source_window_sizes() {
    let viewport = Vec2::new(851.0, 393.0);
    let safe = Vec4::new(12.0, 8.0, 22.0, 12.0);
    for density in [1.0, 2.0, 2.75, 3.0] {
        for scale in [0.35, 0.5, 0.75, 1.0] {
            for count in [0, 1, 2, 4, 7, 11] {
                let mut f = fixture(viewport, density, scale, safe, 170.0, count, true);
                f.app.update();
                f.assert_rows(count.saturating_sub(4));
                f.assert_focused_bounds(safe, viewport, 170.0, density);
            }
        }
    }
}

#[test]
fn keyboard_resize_reflows_available_suffix_and_restores_it_without_rewriting_content() {
    let viewport = Vec2::new(851.0, 393.0);
    let safe = Vec4::new(12.0, 8.0, 22.0, 12.0);
    let mut f = fixture(viewport, 2.75, 0.51, safe, 0.0, 4, true);
    for (ime, first) in [(0.0, 0), (170.0, 0), (230.0, 3), (0.0, 0)] {
        f.app
            .world_mut()
            .resource_mut::<crate::shared_shell::HostState>()
            .ime_bottom = ime * 2.75;
        f.app.update();
        f.assert_rows(first);
        f.assert_focused_bounds(safe, viewport, ime, 2.75);
        assert_eq!(
            f.app.world().get::<Text>(f.input).unwrap().0,
            "unsent draft"
        );
    }
}

#[test]
fn no_available_history_space_hides_rows_without_drawing_them_over_controls_or_ime() {
    let viewport = Vec2::new(568.0, 262.0);
    let safe = Vec4::new(12.0, 8.0, 12.0, 12.0);
    let mut f = fixture(viewport, 1.0, 0.4, safe, 128.0, 7, true);
    f.app.update();
    f.assert_rows(7);
    f.assert_focused_bounds(safe, viewport, 128.0, 1.0);
}

#[test]
fn unfocused_phone_keeps_latest_one_or_two_rows_and_original_settings() {
    for (viewport, first) in [(Vec2::new(851.0, 393.0), 5), (Vec2::new(568.0, 270.0), 6)] {
        let mut f = fixture(viewport, 1.0, 0.5, Vec4::ZERO, 0.0, 7, false);
        let settings = f
            .app
            .world()
            .resource::<NativePlayerUiState>()
            .core
            .chat_settings;
        f.app.update();
        f.assert_rows(first);
        assert_eq!(
            f.app
                .world()
                .resource::<NativePlayerUiState>()
                .core
                .chat_settings,
            settings
        );
        for (entity, action) in &f.controls {
            assert_eq!(
                f.app.world().get::<Node>(*entity).unwrap().display,
                if phone_filter_index(*action).is_some() {
                    Display::None
                } else {
                    Display::Flex
                }
            );
        }
    }
}
