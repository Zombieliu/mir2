//! Text counterparts of Crystal Help/0.png through Help/41.png.
//!
//! Source order: Crystal Client/MirScenes/Dialogs/HelpDialog.cs::LoadImagePages.
//! The catalog was checked against all 42 exported images, not inferred from
//! their repeated page headings. English keeps those original images. Other
//! languages use real UI text so there is no English image behind a translation.
//! Source HelpDialog.cs SHA256:
//! a01fe452a014f3fc23705216c0e6d8a146f9f53edd95df2cd3abc35721311f87.
//! Images 0..41: SHA256 of lines `index.png <lowercase SHA256>\n`, ascending:
//! 09650b772a004f071f78d9609a488a38826f7b27b63b8d513af44ba6f73e5d42.
//! Awakening pages retain the original rules but identify unavailable ordinary
//! client operations; they do not instruct players to use admin commands.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::text::{LineBreak, LineHeight};
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use crate::crystal_ui::typography::crystal_text_font;
use crate::native_i18n::{self, Locale};

const IMAGE_PAGE_COUNT: u8 = 42;
const VIEWPORT_WIDTH: f32 = 512.0;
const VIEWPORT_HEIGHT: f32 = 374.0;
const SCROLL_LINE: f32 = 45.0;

#[derive(Component)]
pub(super) struct LocalizedHelpViewport {
    page: u8,
    locale: Locale,
}

/// Bounded display state, independent of the overlay entity's lifetime.
/// Each translated page keeps its own position; no account data is retained.
#[derive(Resource)]
pub(super) struct HelpScrollState {
    offsets: [[f32; IMAGE_PAGE_COUNT as usize]; 2],
}

impl Default for HelpScrollState {
    fn default() -> Self {
        Self {
            offsets: [[0.0; IMAGE_PAGE_COUNT as usize]; 2],
        }
    }
}

impl LocalizedHelpViewport {
    fn key(&self) -> Option<(usize, usize)> {
        let locale = match self.locale {
            Locale::TraditionalChinese => 0,
            Locale::BrazilianPortuguese => 1,
            Locale::English => return None,
        };
        Some((locale, usize::from(image_page(self.page)?)))
    }
}

impl HelpScrollState {
    fn get(&self, viewport: &LocalizedHelpViewport) -> f32 {
        viewport
            .key()
            .map_or(0.0, |(locale, page)| self.offsets[locale][page])
    }

    fn set(&mut self, viewport: &LocalizedHelpViewport, value: f32) {
        if value.is_finite() {
            if let Some((locale, page)) = viewport.key() {
                self.offsets[locale][page] = value.max(0.0);
            }
        }
    }
}

fn image_page(page: u8) -> Option<u8> {
    page.checked_sub(3)
        .filter(|index| *index < IMAGE_PAGE_COUNT)
}

/// Return true only when this function replaces an original Help image.
/// The first three shortcut pages and the inactive/English host stay unchanged.
pub(super) fn render(parent: &mut ChildSpawnerCommands, page: u8) -> bool {
    if !native_i18n::active() || native_i18n::locale() == Locale::English {
        return false;
    }
    let Some(index) = image_page(page) else {
        return false;
    };
    let title = native_i18n::key(&format!("help.page.{index:02}.title"), "");
    let body = native_i18n::key(&format!("help.page.{index:02}.body"), "");
    // Long pages stay readable; actual overflow is scrollable rather than
    // repeatedly shrinking text or silently cropping the end of the guide.
    let size = if body.chars().count() > 900 {
        13.0
    } else {
        14.0
    };
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(75.0),
                width: Val::Px(VIEWPORT_WIDTH),
                height: Val::Px(396.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.02, 0.01, 0.98)),
            FocusPolicy::Block,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Px(VIEWPORT_WIDTH),
                        height: Val::Px(VIEWPORT_HEIGHT),
                        padding: UiRect::all(Val::Px(12.0)),
                        overflow: Overflow::scroll_y(),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                    FocusPolicy::Block,
                    LocalizedHelpViewport {
                        page,
                        locale: native_i18n::locale(),
                    },
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new(title),
                        crystal_text_font(16.0),
                        TextColor(Color::srgb(1.0, 0.82, 0.2)),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::right(Val::Px(4.0)),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                    parent.spawn((
                        Text::new(body),
                        crystal_text_font(size),
                        TextColor(Color::srgb(0.94, 0.94, 0.88)),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                        LineHeight::Px(size * 1.45),
                        Node {
                            width: Val::Percent(100.0),
                            // Leave a small glyph/word-spacing guard within the
                            // paragraph box at the scroll viewport's right edge.
                            padding: UiRect::right(Val::Px(4.0)),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                });
            parent.spawn((
                Text::new(native_i18n::key("help.scroll_hint", "")),
                crystal_text_font(11.0),
                TextColor(Color::srgb(0.75, 0.73, 0.64)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    top: Val::Px(378.0),
                    width: Val::Px(488.0),
                    height: Val::Px(18.0),
                    ..default()
                },
            ));
        });
    true
}

fn clamped_scroll(current: f32, delta: f32, maximum: f32) -> f32 {
    if !delta.is_finite() || !current.is_finite() || !maximum.is_finite() {
        return current;
    }
    (current - delta).clamp(0.0, maximum.max(0.0))
}

fn laid_out_scroll_maximum(computed: &ComputedNode) -> Option<f32> {
    // New overlay children have default (zero) measurements until Layout. They
    // must not overwrite a saved position simply because they are new entities.
    if computed.is_empty()
        || computed.content_size().x <= 0.0
        || computed.content_size().y <= 0.0
        || computed.inverse_scale_factor <= 0.0
    {
        return None;
    }
    let maximum = (computed.content_size().y - computed.size().y) * computed.inverse_scale_factor;
    maximum.is_finite().then_some(maximum.max(0.0))
}

/// Register alongside the native overlay's existing MouseWheel reader.
/// Each reader is independent; only the hovered help viewport changes here.
pub(super) fn scroll_help(
    mut wheel: MessageReader<MouseWheel>,
    mut state: ResMut<HelpScrollState>,
    mut viewports: Query<(
        &LocalizedHelpViewport,
        &RelativeCursorPosition,
        &ComputedNode,
        &mut ScrollPosition,
    )>,
) {
    let delta: f32 = wheel
        .read()
        .filter_map(|event| {
            let value = match event.unit {
                MouseScrollUnit::Line => event.y * SCROLL_LINE,
                MouseScrollUnit::Pixel => event.y,
            };
            value.is_finite().then_some(value)
        })
        .sum();
    if delta == 0.0 {
        return;
    }
    for (viewport, cursor, computed, mut scroll) in &mut viewports {
        if !cursor.cursor_over() {
            continue;
        }
        let Some(maximum) = laid_out_scroll_maximum(computed) else {
            continue;
        };
        scroll.y = clamped_scroll(scroll.y, delta, maximum);
        state.set(viewport, scroll.y);
    }
}

/// Update: run after `scroll_help` and before the overlay rebuild/despawn.
/// Record Layout's actual clamping only once the old viewport has been laid out.
pub(super) fn capture_help_scroll(
    mut state: ResMut<HelpScrollState>,
    viewports: Query<(&LocalizedHelpViewport, &ComputedNode, &ScrollPosition)>,
) {
    for (viewport, computed, scroll) in &viewports {
        if let Some(maximum) = laid_out_scroll_maximum(computed) {
            state.set(viewport, clamped_scroll(scroll.y, 0.0, maximum));
        }
    }
}

/// PostUpdate: run before `UiSystems::Layout`, after Update's deferred spawns.
/// Do not clamp against a newly spawned viewport's zero measurements. Layout
/// will resolve the offset against its real content; Update records that result.
pub(super) fn restore_help_scroll(
    state: Res<HelpScrollState>,
    mut viewports: Query<(&LocalizedHelpViewport, &mut ScrollPosition)>,
) {
    for (viewport, mut scroll) in &mut viewports {
        scroll.y = state.get(viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn catalog() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../../packages/game-data/data/native-i18n/help.json"
        ))
        .unwrap()
    }

    #[test]
    fn localized_help_covers_each_original_image_once_and_preserves_shortcut_pages() {
        for page in 0..3 {
            assert_eq!(image_page(page), None);
        }
        for page in 3..45 {
            assert_eq!(image_page(page), Some(page - 3));
        }
        for page in 45..=255 {
            assert_eq!(image_page(page), None);
        }
        let catalog = catalog();
        let entries = catalog["entries"].as_array().unwrap();
        for language in ["en", "zh-TW", "pt-BR"] {
            let mut unique = HashSet::new();
            for page in 0..IMAGE_PAGE_COUNT {
                for suffix in ["title", "body"] {
                    let id = format!("help.page.{page:02}.{suffix}");
                    let matching: Vec<_> = entries.iter().filter(|e| e["key"] == id).collect();
                    assert_eq!(matching.len(), 1, "{id}");
                    let text = matching[0][language].as_str().unwrap();
                    assert!(!text.trim().is_empty(), "{language}: {id}");
                    if suffix == "body" {
                        assert!(text.chars().count() > 60, "{language}: {id}");
                        assert!(unique.insert(text), "generic duplicate help: {id}");
                    }
                }
            }
        }
        assert_eq!(
            entries
                .iter()
                .filter(|e| e["key"].as_str().unwrap().starts_with("help.shortcut."))
                .count(),
            39
        );
    }

    #[test]
    fn localized_help_catalog_is_integrated_for_both_translated_languages() {
        for language in [Locale::TraditionalChinese, Locale::BrazilianPortuguese] {
            for page in 0..IMAGE_PAGE_COUNT {
                for suffix in ["title", "body"] {
                    let id = format!("help.page.{page:02}.{suffix}");
                    assert!(
                        !native_i18n::for_locale(language, &id, "").is_empty(),
                        "{id}"
                    );
                }
            }
            assert!(!native_i18n::for_locale(language, "help.scroll_hint", "").is_empty());
        }
    }

    #[test]
    fn localized_help_scroll_reaches_both_ends_and_never_leaves_content_bounds() {
        assert_eq!(clamped_scroll(0.0, -SCROLL_LINE, 250.0), 45.0);
        assert_eq!(clamped_scroll(200.0, -SCROLL_LINE * 8.0, 250.0), 250.0);
        assert_eq!(clamped_scroll(250.0, SCROLL_LINE * 8.0, 250.0), 0.0);
        assert_eq!(clamped_scroll(100.0, -SCROLL_LINE, -20.0), 0.0);
        assert_eq!(clamped_scroll(20.0, f32::NAN, 250.0), 20.0);
    }

    fn scrolling_fixture() -> App {
        let mut app = App::new();
        app.init_resource::<HelpScrollState>()
            .add_systems(Update, capture_help_scroll)
            .add_systems(PostUpdate, restore_help_scroll);
        app
    }

    fn measured_viewport() -> ComputedNode {
        ComputedNode {
            size: Vec2::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT),
            content_size: Vec2::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT + 500.0),
            inverse_scale_factor: 1.0,
            ..default()
        }
    }

    fn spawn_viewport(app: &mut App, page: u8, locale: Locale, y: f32, laid_out: bool) -> Entity {
        app.world_mut()
            .spawn((
                LocalizedHelpViewport { page, locale },
                if laid_out {
                    measured_viewport()
                } else {
                    ComputedNode::default()
                },
                ScrollPosition(Vec2::new(0.0, y)),
            ))
            .id()
    }

    #[test]
    fn localized_help_scroll_survives_despawn_and_respawn_of_the_same_page() {
        let mut app = scrolling_fixture();
        let old = spawn_viewport(&mut app, 3, Locale::TraditionalChinese, 180.0, true);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(old).unwrap().y, 180.0);
        app.world_mut().despawn(old);

        let rebuilt = spawn_viewport(&mut app, 3, Locale::TraditionalChinese, 0.0, false);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(rebuilt).unwrap().y, 180.0);
        app.world_mut()
            .entity_mut(rebuilt)
            .insert(measured_viewport());
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(rebuilt).unwrap().y, 180.0);
    }

    #[test]
    fn localized_help_unlaid_out_zero_content_cannot_erase_the_saved_offset() {
        let mut app = scrolling_fixture();
        let old = spawn_viewport(&mut app, 4, Locale::BrazilianPortuguese, 240.0, true);
        app.update();
        app.world_mut().despawn(old);
        let rebuilt = spawn_viewport(&mut app, 4, Locale::BrazilianPortuguese, 0.0, false);
        // Simulate Layout waiting for text/font measurements for several frames.
        for _ in 0..3 {
            app.world_mut()
                .get_mut::<ScrollPosition>(rebuilt)
                .unwrap()
                .y = 0.0;
            app.update();
            assert_eq!(app.world().get::<ScrollPosition>(rebuilt).unwrap().y, 240.0);
        }
        // Once there is real content, a genuine shorter extent may clamp it.
        let mut shorter = measured_viewport();
        shorter.content_size.y = VIEWPORT_HEIGHT + 90.0;
        app.world_mut().entity_mut(rebuilt).insert(shorter);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(rebuilt).unwrap().y, 90.0);
    }

    #[test]
    fn localized_help_scroll_positions_are_independent_across_page_and_locale() {
        let mut app = scrolling_fixture();
        let first = spawn_viewport(&mut app, 3, Locale::TraditionalChinese, 160.0, true);
        app.update();
        app.world_mut().despawn(first);
        let other_page = spawn_viewport(&mut app, 4, Locale::TraditionalChinese, 0.0, false);
        app.update();
        assert_eq!(
            app.world().get::<ScrollPosition>(other_page).unwrap().y,
            0.0
        );
        app.world_mut()
            .entity_mut(other_page)
            .insert(measured_viewport());
        app.world_mut()
            .get_mut::<ScrollPosition>(other_page)
            .unwrap()
            .y = 80.0;
        app.update();
        app.world_mut().despawn(other_page);
        let other_locale = spawn_viewport(&mut app, 3, Locale::BrazilianPortuguese, 0.0, false);
        app.update();
        assert_eq!(
            app.world().get::<ScrollPosition>(other_locale).unwrap().y,
            0.0
        );
        app.world_mut()
            .entity_mut(other_locale)
            .insert(measured_viewport());
        app.world_mut()
            .get_mut::<ScrollPosition>(other_locale)
            .unwrap()
            .y = 220.0;
        app.update();
        app.world_mut().despawn(other_locale);
        for (page, locale, expected) in [
            (3, Locale::TraditionalChinese, 160.0),
            (4, Locale::TraditionalChinese, 80.0),
            (3, Locale::BrazilianPortuguese, 220.0),
        ] {
            let reopened = spawn_viewport(&mut app, page, locale, 0.0, false);
            app.update();
            assert_eq!(
                app.world().get::<ScrollPosition>(reopened).unwrap().y,
                expected
            );
            app.world_mut().despawn(reopened);
        }
    }
}
