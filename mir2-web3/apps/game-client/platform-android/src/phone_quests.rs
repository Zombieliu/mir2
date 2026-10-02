//! Fit the actual shared diary/detail pair as one Android presentation group.
//! The host supplies a dp workspace to the optional shared phone renderer.
//! Shared controllers/models stay authoritative; the common transform retains
//! independent diary/detail windows and the same HUD/thumb lane as the HUD.
use crate::shared_shell::{AndroidStageFit, HostState};
use bevy::prelude::*;
#[cfg(feature = "ui-preview")]
use bevy::text::TextLayoutInfo;
use mir2_client_bevy::{
    crystal_ui::overlays::{NativePlayerUiSet, NativePlayerUiState},
    native_shell::{NativeShellModel, NativeShellScreen},
    quest_ui::{
        PhoneQuestPresentation, QuestConfirmationPanel, QuestDetailPanel, QuestLogPanel, QuestUiRoot,
    },
};

pub(crate) fn install(app: &mut App) {
    app.init_resource::<PhoneQuestPresentation>()
        .add_systems(
            Update,
            publish_phone_presentation
                .after(NativePlayerUiSet::Mutate)
                .before(NativePlayerUiSet::Read),
        )
        .add_systems(
            PostUpdate,
            fit_shared_quest_windows
                .after(AndroidStageFit)
                .before(bevy::ui::UiSystems::Layout),
        );
    #[cfg(feature = "ui-preview")]
    app.add_systems(
        PostUpdate,
        report_phone_quest_geometry.after(bevy::ui::UiSystems::PostLayout),
    );
}

fn publish_phone_presentation(
    windows: Query<&Window>,
    host: Res<HostState>,
    player: Res<NativePlayerUiState>,
    world_input: crate::world_input::WorldInputContext,
    mut presentation: ResMut<PhoneQuestPresentation>,
) {
    let next = windows
        .single()
        .ok()
        .and_then(|window| {
            let viewport = Vec2::new(window.width(), window.height());
            let fit =
                mir2_client_bevy::crystal_ui::CrystalStageTransform::fit(viewport.x, viewport.y);
            let safe = host.quest_presentation_insets() / window.scale_factor();
            let workspace = workspace(
                viewport,
                safe,
                host.ime_bottom / window.scale_factor(),
                world_input.quest_sidebar_requested(&player),
            )?;
            Some(PhoneQuestPresentation {
                workspace: workspace.size(),
                authored_unit: 1.0 / fit.scale,
            })
        })
        .unwrap_or_default();
    if *presentation != next {
        *presentation = next;
    }
}

#[cfg(feature = "ui-preview")]
fn report_phone_quest_geometry(
    windows: Query<&Window>,
    stage: Res<UiScale>,
    texts: Query<
        (&Text, &TextFont, &TextLayoutInfo),
        With<mir2_client_bevy::quest_ui::PhoneQuestText>,
    >,
    controls: Query<
        (&ComputedNode, &bevy::ui::UiGlobalTransform),
        With<mir2_client_bevy::quest_ui::PhoneQuestControl>,
    >,
    areas: Query<&ComputedNode, With<mir2_client_bevy::quest_ui::PhoneQuestScrollArea>>,
    mut frames: Local<u16>,
) {
    *frames = frames.saturating_add(1);
    if *frames != 90 {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let density = window.scale_factor();
    for (text, font, layout) in &texts {
        let FontSize::Px(size) = font.font_size else {
            continue;
        };
        info!(text=?text.0, font_dp=size*stage.0, glyphs=layout.glyphs.len(),
            alpha_masks=layout.glyphs.iter().filter(|glyph| glyph.atlas_info.is_alpha_mask).count(),
            "ANDROID_PHONE_QUEST_TEXT");
    }
    for (node, transform) in &controls {
        info!(size_dp=?node.size()/density, center_dp=?transform.translation/density,
            "ANDROID_PHONE_QUEST_CONTROL");
    }
    for node in &areas {
        info!(size_dp=?node.size()/density, content_dp=?node.content_size()/density,
            offset=?node.scroll_position*node.inverse_scale_factor,
            "ANDROID_PHONE_QUEST_SCROLL");
    }
}

fn workspace(viewport: Vec2, safe: Vec4, ime: f32, protect_chrome: bool) -> Option<Rect> {
    if !viewport.is_finite()
        || !safe.is_finite()
        || !ime.is_finite()
        || ime < 0.0
        || viewport.min_element() <= 0.0
    {
        return None;
    }
    let min = Vec2::new(safe.x, safe.y).max(Vec2::ZERO).min(viewport);
    let max = (viewport - Vec2::new(safe.z, safe.w).max(Vec2::ZERO))
        .max(min)
        .min(viewport);
    let available = max - min;
    if available.min_element() <= 0.0 {
        return None;
    }
    let gutter = Vec2::splat(16.0).min(available * 0.25);
    Some(
        crate::phone_panels::panel_sidebar(viewport, safe.max(Vec4::ZERO), ime, protect_chrome)
            .map(|sidebar| sidebar.workspace)
            .unwrap_or_else(|| Rect::from_corners(min + gutter, max - gutter)),
    )
}

#[derive(Clone, Copy, Debug)]
struct GroupFocus {
    group: Rect,
    target_center: Vec2,
    scale: f32,
}

fn focus_group(
    group: Rect,
    viewport: Vec2,
    safe: Vec4,
    stage_scale: f32,
    root_origin: Vec2,
    ime: f32,
    protect_chrome: bool,
) -> Option<GroupFocus> {
    if !viewport.is_finite()
        || !safe.is_finite()
        || !group.min.is_finite()
        || !group.max.is_finite()
        || !root_origin.is_finite()
        || !stage_scale.is_finite()
        || !ime.is_finite()
        || ime < 0.0
        || stage_scale <= 0.0
        || viewport.min_element() <= 0.0
        || group.size().min_element() <= 0.0
    {
        return None;
    }
    let workspace = workspace(viewport, safe, ime, protect_chrome)?;
    let scale = (workspace.size() / (group.size() * stage_scale))
        .min_element()
        .min(3.2);
    Some(GroupFocus {
        group,
        target_center: workspace.center() / stage_scale - root_origin,
        scale,
    })
}

impl GroupFocus {
    fn transform(self, panel: Rect) -> UiTransform {
        // Bevy scales each Node around its own center, not the group origin.
        // Shift each center as well so two independent shared windows retain
        // their source gap instead of growing on top of one another.
        let center = panel.center();
        let target = self.target_center + (center - self.group.center()) * self.scale;
        UiTransform {
            translation: Val2::px(target.x - center.x, target.y - center.y),
            scale: Vec2::splat(self.scale),
            ..default()
        }
    }
}

fn panel_rect(node: &Node) -> Option<Rect> {
    let (Val::Px(x), Val::Px(y), Val::Px(w), Val::Px(h)) =
        (node.left, node.top, node.width, node.height)
    else {
        return None;
    };
    let rect = Rect::from_corners(Vec2::new(x, y), Vec2::new(x + w, y + h));
    (w > 0.0 && h > 0.0 && rect.min.is_finite() && rect.max.is_finite()).then_some(rect)
}

fn fit_shared_quest_windows(
    windows: Query<&Window>,
    host: Res<HostState>,
    shell: Res<NativeShellModel>,
    player: Res<NativePlayerUiState>,
    world_input: crate::world_input::WorldInputContext,
    stage: Res<UiScale>,
    roots: Query<
        &Node,
        (
            With<QuestUiRoot>,
            Without<QuestLogPanel>,
            Without<QuestDetailPanel>,
            Without<QuestConfirmationPanel>,
        ),
    >,
    mut panels: Query<
        (&Node, &mut UiTransform),
        (
            Or<(
                With<QuestLogPanel>,
                With<QuestDetailPanel>,
                With<QuestConfirmationPanel>,
            )>,
            Without<QuestUiRoot>,
        ),
    >,
) {
    let window = windows.single().ok();
    let root_origin = roots.single().ok().and_then(|node| {
        let (Val::Px(x), Val::Px(y)) = (node.left, node.top) else {
            return None;
        };
        Some(Vec2::new(x, y))
    });
    let group = panels
        .iter()
        .filter_map(|(node, _)| {
            (node.display != Display::None)
                .then(|| panel_rect(node))
                .flatten()
        })
        .reduce(|a, b| Rect::from_corners(a.min.min(b.min), a.max.max(b.max)));
    let focus = if shell.screen == NativeShellScreen::InGame {
        window
            .zip(root_origin)
            .zip(group)
            .and_then(|((window, root), group)| {
                // Host insets are physical pixels; Window/UI coordinates are logical.
                let density = window.scale_factor();
                let safe = host.quest_presentation_insets() / density;
                focus_group(
                    group,
                    Vec2::new(window.width(), window.height()),
                    safe,
                    stage.0,
                    root,
                    host.ime_bottom / density,
                    world_input.quest_sidebar_requested(&player),
                )
            })
    } else {
        None
    };
    for (node, mut transform) in &mut panels {
        *transform = focus
            .filter(|_| node.display != Display::None)
            .zip(panel_rect(node))
            .map(|(focus, rect)| focus.transform(rect))
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::crystal_ui::CrystalStageTransform;
    use mir2_client_bevy::quest_ui::*;

    fn diary() -> Rect {
        Rect::from_corners(
            Vec2::new(QUEST_DIARY_DESIGN_LEFT, QUEST_DIARY_DESIGN_TOP),
            Vec2::new(
                QUEST_DIARY_DESIGN_LEFT + QUEST_DIARY_DESIGN_WIDTH,
                QUEST_DIARY_DESIGN_TOP + QUEST_DIARY_DESIGN_HEIGHT,
            ),
        )
    }
    fn detail() -> Rect {
        Rect::from_corners(
            Vec2::new(QUEST_DETAIL_DESIGN_LEFT, QUEST_DETAIL_DESIGN_TOP),
            Vec2::new(
                QUEST_DETAIL_DESIGN_LEFT + QUEST_DETAIL_DESIGN_WIDTH,
                QUEST_DETAIL_DESIGN_TOP + QUEST_DETAIL_DESIGN_HEIGHT,
            ),
        )
    }
    fn screen_rect(panel: Rect, transform: UiTransform, root: Vec2, scale: f32) -> Rect {
        let (Val::Px(x), Val::Px(y)) = (transform.translation.x, transform.translation.y) else {
            panic!()
        };
        let center = (panel.center() + Vec2::new(x, y) + root) * scale;
        let half = panel.size() * transform.scale * scale * 0.5;
        Rect::from_corners(center - half, center + half)
    }

    #[test]
    fn shared_pair_retains_gap_and_fits_safe_ime_viewport() {
        let pair = Rect::from_corners(diary().min.min(detail().min), diary().max.max(detail().max));
        for viewport in [
            Vec2::new(851.0, 393.0),
            Vec2::new(568.0, 262.0),
            Vec2::new(1600.0, 720.0),
        ] {
            for safe in [Vec4::ZERO, Vec4::new(24.0, 14.0, 18.0, 110.0)] {
                let fit = CrystalStageTransform::fit(viewport.x, viewport.y);
                // Use an already IME-panned root, not an assumed centered stage.
                let root = Vec2::new(fit.offset_x / fit.scale, -87.0);
                let focus = focus_group(pair, viewport, safe, fit.scale, root, 0.0, true).unwrap();
                let a = screen_rect(diary(), focus.transform(diary()), root, fit.scale);
                let b = screen_rect(detail(), focus.transform(detail()), root, fit.scale);
                assert!(
                    a.max.x < b.min.x,
                    "Do not overlap independently centered quest windows"
                );
                for rect in [a, b] {
                    assert!(rect.min.x >= safe.x - 0.001 && rect.min.y >= safe.y - 0.001);
                    assert!(rect.max.x <= viewport.x - safe.z + 0.001);
                    assert!(rect.max.y <= viewport.y - safe.w + 0.001);
                }
            }
        }
    }

    #[test]
    fn single_diary_is_larger_than_the_unfocused_desktop_canvas() {
        let viewport = Vec2::new(851.0, 393.0);
        let fit = CrystalStageTransform::fit(viewport.x, viewport.y);
        let focus = focus_group(
            diary(),
            viewport,
            Vec4::ZERO,
            fit.scale,
            Vec2::ZERO,
            0.0,
            true,
        )
        .unwrap();
        assert!(focus.scale > 1.4 && focus.scale <= 3.2);
    }

    #[test]
    fn shared_quest_pair_stays_in_the_same_protected_workspace_as_phone_hud() {
        let pair = Rect::from_corners(diary().min.min(detail().min), diary().max.max(detail().max));
        for (viewport, safe) in [
            (Vec2::new(851.0, 393.0), Vec4::ZERO),
            (Vec2::new(851.0, 393.0), Vec4::new(12.0, 8.0, 22.0, 12.0)),
            (Vec2::new(960.0, 432.0), Vec4::new(24.0, 12.0, 24.0, 12.0)),
        ] {
            let sidebar = crate::phone_panels::panel_sidebar(viewport, safe, 0.0, true).unwrap();
            let fit = CrystalStageTransform::fit(viewport.x, viewport.y);
            let root = Vec2::new(fit.offset_x / fit.scale, fit.offset_y / fit.scale);
            let focus = focus_group(pair, viewport, safe, fit.scale, root, 0.0, true).unwrap();
            for panel in [diary(), detail()] {
                let rect = screen_rect(panel, focus.transform(panel), root, fit.scale);
                assert!(
                    rect.min.x >= sidebar.workspace.min.x - 0.001
                        && rect.min.y >= sidebar.workspace.min.y - 0.001
                        && rect.max.x <= sidebar.workspace.max.x + 0.001
                        && rect.max.y <= sidebar.workspace.max.y + 0.001,
                    "Quest windows must leave the HUD, six belt targets, chat and thumbs unobscured: {rect:?} vs {:?}",
                    sidebar.workspace
                );
                for chrome in [sidebar.status, sidebar.belt, sidebar.chat] {
                    assert!(rect.intersect(chrome).is_empty());
                }
            }
        }
    }

    #[test]
    fn malformed_or_unavailable_geometry_has_no_focus() {
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(focus_group(
                diary(),
                Vec2::new(851.0, 393.0),
                Vec4::ZERO,
                scale,
                Vec2::ZERO,
                0.0,
                true
            )
            .is_none());
        }
        assert!(focus_group(
            diary(),
            Vec2::new(851.0, 393.0),
            Vec4::new(900.0, 0.0, 0.0, 0.0),
            1.0,
            Vec2::ZERO,
            0.0,
            true
        )
        .is_none());
        assert!(panel_rect(&Node::default()).is_none());
    }

    #[test]
    fn compact_and_ime_are_safe_fallbacks_not_protected_chrome_acceptance() {
        let pair = Rect::from_corners(diary().min.min(detail().min), diary().max.max(detail().max));
        for (viewport, safe, ime) in [
            (Vec2::new(568.0, 262.0), Vec4::ZERO, 0.0),
            (
                Vec2::new(851.0, 393.0),
                Vec4::new(0.0, 0.0, 0.0, 170.0),
                170.0,
            ),
        ] {
            assert!(crate::phone_panels::panel_sidebar(viewport, safe, ime, true).is_none());
            let fit = CrystalStageTransform::fit(viewport.x, viewport.y);
            let fallback =
                focus_group(pair, viewport, safe, fit.scale, Vec2::ZERO, ime, true).unwrap();
            let unprotected =
                focus_group(pair, viewport, safe, fit.scale, Vec2::ZERO, ime, false).unwrap();
            assert_eq!(fallback.target_center, unprotected.target_center);
            assert_eq!(fallback.scale, unprotected.scale);
            for panel in [diary(), detail()] {
                let rect = screen_rect(panel, fallback.transform(panel), Vec2::ZERO, fit.scale);
                assert!(rect.min.x >= safe.x && rect.min.y >= safe.y);
                assert!(rect.max.x <= viewport.x - safe.z + 0.001);
                assert!(rect.max.y <= viewport.y - safe.w + 0.001);
            }
        }
        for ime in [f32::NAN, f32::INFINITY, -1.0] {
            assert!(focus_group(
                diary(),
                Vec2::new(851.0, 393.0),
                Vec4::ZERO,
                1.0,
                Vec2::ZERO,
                ime,
                true
            )
            .is_none());
        }
    }

    #[test]
    fn phone_reflow_geometry_preserves_dp_targets_without_secondary_shrink() {
        for (viewport, safe, ime) in [
            (Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0),
            (
                Vec2::new(960.0, 432.0),
                Vec4::new(12.0, 8.0, 22.0, 12.0),
                0.0,
            ),
            (Vec2::new(568.0, 262.0), Vec4::ZERO, 0.0),
            (
                Vec2::new(851.0, 393.0),
                Vec4::new(0.0, 0.0, 0.0, 170.0),
                170.0,
            ),
        ] {
            let expected = workspace(viewport, safe, ime, true).unwrap();
            let fit = CrystalStageTransform::fit(viewport.x, viewport.y);
            let authored = Rect::from_corners(Vec2::ZERO, expected.size() / fit.scale);
            let focused =
                focus_group(authored, viewport, safe, fit.scale, Vec2::ZERO, ime, true).unwrap();
            assert!((focused.scale - 1.0).abs() < 0.0001);
            let result = screen_rect(authored, focused.transform(authored), Vec2::ZERO, fit.scale);
            assert!(result.min.distance(expected.min) < 0.001);
            assert!(result.max.distance(expected.max) < 0.001);
            assert!((48.0 / fit.scale * fit.scale * focused.scale - 48.0).abs() < 0.001);
        }
    }

    #[test]
    fn host_publishes_workspace_and_density_conversion_to_shared_phone_renderer() {
        let mut app = App::new();
        let mut player = NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        app.insert_resource(player)
            .init_resource::<HostState>()
            .init_resource::<PhoneQuestPresentation>()
            .add_systems(Update, publish_phone_presentation);
        app.world_mut().spawn(Window {
            resolution: bevy::window::WindowResolution::new(851, 393),
            ..default()
        });
        app.update();
        let value = app.world().resource::<PhoneQuestPresentation>();
        assert_eq!(
            value.workspace,
            workspace(Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0, true)
                .unwrap()
                .size()
        );
        assert!(
            (value.authored_unit * CrystalStageTransform::fit(851.0, 393.0).scale - 1.0).abs()
                < 0.001
        );
    }

    #[test]
    fn actual_system_resets_hidden_and_out_of_game_transforms() {
        let mut app = App::new();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        let mut player = NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        app.insert_resource(HostState::default())
            .insert_resource(shell)
            .insert_resource(player)
            .insert_resource(UiScale(0.51))
            .add_systems(Update, fit_shared_quest_windows);
        app.world_mut().spawn(Window {
            resolution: bevy::window::WindowResolution::new(851, 393),
            ..default()
        });
        app.world_mut().spawn((
            QuestUiRoot,
            Node {
                left: px(0.0),
                top: px(0.0),
                ..default()
            },
        ));
        let entity = app
            .world_mut()
            .spawn((
                QuestLogPanel,
                Node {
                    left: px(QUEST_DIARY_DESIGN_LEFT),
                    top: px(QUEST_DIARY_DESIGN_TOP),
                    width: px(QUEST_DIARY_DESIGN_WIDTH),
                    height: px(QUEST_DIARY_DESIGN_HEIGHT),
                    ..default()
                },
            ))
            .id();
        app.update();
        assert!(app.world().get::<UiTransform>(entity).unwrap().scale.x > 1.4);
        app.world_mut().get_mut::<Node>(entity).unwrap().display = Display::None;
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(entity).unwrap().scale,
            Vec2::ONE
        );
        app.world_mut().get_mut::<Node>(entity).unwrap().display = Display::Flex;
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login;
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(entity).unwrap().scale,
            Vec2::ONE
        );
    }
}
