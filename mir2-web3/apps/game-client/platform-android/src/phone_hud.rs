//! Phone-only presentation of the shared read model and real Crystal controls.
//! No item, combat, chat-filter or save rule is implemented by this adapter.
use crate::{android_input::AndroidShellState, shared_shell::AndroidStageFit};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
#[cfg(feature = "ui-preview")]
use bevy::text::TextLayoutInfo;
use mir2_client_bevy::{
    crystal_ui::{chat::*, hud::*, overlays::NativePlayerUiState, typography::crystal_text_font},
    native_shell::{NativeShellModel, NativeShellScreen},
    read_model::UiReadModel,
};

const TAP: f32 = 48.0;
const ROW: f32 = 18.0;
#[derive(Component)]
struct PhoneStatus;
#[derive(Component, Clone, Copy)]
enum StatusLabel {
    Name,
    Hp,
    Mp,
    Footer,
}
#[derive(Component, Clone, Copy)]
enum StatusBar {
    Hp,
    Mp,
}
#[derive(Component)]
struct ChatEntry;
#[derive(Component)]
struct ChatEntryLabel;
#[derive(Component)]
struct PhoneChatControlLabel;

#[derive(Clone, Copy, Debug)]
struct PhoneLayout {
    status: Rect,
    chat: Rect,
    belt: Rect,
    unit: f32,
}

/// Logical window coordinates, not authored desktop pixels. Insets and IME are
/// physical Android values divided by density by the caller exactly once.
fn phone_layout(
    viewport: Vec2,
    safe: Vec4,
    ime: f32,
    scale: f32,
    focused: bool,
    rows: usize,
) -> PhoneLayout {
    let left = safe.x + 16.0;
    let right = (viewport.x - safe.z - 16.0).max(left + 1.0);
    let mut bottom = (viewport.y - safe.w.max(ime) - 12.0).max(safe.y + 1.0);
    let width = right - left;
    // Thumb zones stay clear while playing; typing may use the whole safe width.
    let thumbs = crate::mobile_ui::thumb_footprints(viewport.y);
    let gap = (width - thumbs.x - thumbs.y).max(1.0);
    let lift_chat = !focused && gap < 200.0;
    let chat_width = if focused {
        width.min(620.0)
    } else if lift_chat {
        width.min(360.0)
    } else {
        gap.min(480.0)
    };
    if lift_chat {
        bottom = (bottom - TAP - 14.0).max(safe.y + 1.0);
    }
    let chat_height = if focused {
        ROW * rows.min(4) as f32 + TAP * 2.0 + 12.0
    } else {
        ROW * rows.min(phone_history_rows(viewport.y, false)) as f32 + TAP + 8.0
    };
    let chat_left = if focused || lift_chat {
        left + (width - chat_width) * 0.5
    } else {
        left + thumbs.x + (gap - chat_width) * 0.5
    };
    let chat_top = (bottom - chat_height).max(safe.y + 8.0);
    let belt_width = TAP * 6.0;
    PhoneLayout {
        status: Rect::from_corners(
            Vec2::new(left, safe.y + 32.0),
            Vec2::new(
                left + width.min(224.0),
                safe.y + if viewport.y < 320.0 { 110.0 } else { 130.0 },
            ),
        ),
        chat: Rect::from_corners(
            Vec2::new(chat_left, chat_top),
            Vec2::new(chat_left + chat_width, bottom),
        ),
        belt: Rect::from_corners(
            Vec2::new(left + (width - belt_width) * 0.5, chat_top - TAP - 8.0),
            Vec2::new(left + (width + belt_width) * 0.5, chat_top - 8.0),
        ),
        unit: 1.0 / scale.max(0.01),
    }
}

fn phone_history_rows(height: f32, focused: bool) -> usize {
    if focused {
        4
    } else if height < 320.0 {
        1
    } else {
        2
    }
}

fn rect_node(rect: Rect, unit: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(rect.min.x * unit),
        top: px(rect.min.y * unit),
        width: px(rect.width() * unit),
        height: px(rect.height() * unit),
        ..default()
    }
}

pub(crate) fn install(app: &mut App) {
    app.add_systems(Startup, spawn)
        .add_systems(
            Update,
            chat_entry.after(mir2_client_bevy::crystal_ui::overlays::NativePlayerUiSet::Mutate),
        )
        .add_systems(
            PostUpdate,
            (decorate_belt, fit_phone_hud, use_android_system_fonts)
                .chain()
                .after(AndroidStageFit)
                // New shared chat children must be decorated before camera/
                // font propagation AND stacking, not merely before geometry.
                .before(bevy::ui::UiSystems::Prepare)
                .before(bevy::ui::UiSystems::Stack)
                .before(bevy::ui::UiSystems::Layout),
        );
    #[cfg(feature = "ui-preview")]
    app.add_systems(
        PostUpdate,
        report_phone_text.after(bevy::ui::UiSystems::PostLayout),
    );
}

#[cfg(feature = "ui-preview")]
fn report_phone_text(
    nodes: Query<(
        Option<&StatusLabel>,
        Has<PhoneChatControlLabel>,
        &TextFont,
        &TextColor,
        &TextLayoutInfo,
    )>,
    mut frames: Local<u16>,
) {
    *frames += 1;
    if *frames != 90 {
        return;
    }
    for (label, control, font, color, layout) in &nodes {
        if matches!(label, Some(StatusLabel::Hp)) || control {
            info!(?font,?color,glyph_masks=?layout.glyphs.iter().map(|glyph|glyph.atlas_info.is_alpha_mask).collect::<Vec<_>>(),"ANDROID_PHONE_TEXT_LAYOUT");
        }
    }
}

// Arial is not an Android system family. Unresolved digits may fall back to
// color emoji (intrinsic black), which ignores TextColor. Use Android's actual
// system UI family; do not change desktop typography or explicit asset fonts.
fn use_android_system_fonts(mut fonts: Query<&mut TextFont>) {
    for mut font in &mut fonts {
        if matches!(&font.font, FontSource::Family(name) if name == "Arial") {
            font.font = FontSource::SystemUi;
        }
    }
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            PhoneStatus,
            Node {
                display: Display::None,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.035, 0.04, 0.80)),
            BorderColor::all(Color::srgba(0.66, 0.52, 0.28, 0.8)),
            GlobalZIndex(960),
        ))
        .with_children(|root| {
            for (label, top) in [
                (StatusLabel::Name, 8.0),
                (StatusLabel::Hp, 29.0),
                (StatusLabel::Mp, 51.0),
                (StatusLabel::Footer, 78.0),
            ] {
                root.spawn((
                    label,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(10),
                        top: px(top),
                        ..default()
                    },
                    Text::new(""),
                    crystal_text_font(12.0),
                    TextColor(Color::srgb(0.98, 0.91, 0.76)),
                ));
            }
            for (bar, top, color) in [
                (StatusBar::Hp, 46.0, Color::srgb(0.72, 0.13, 0.10)),
                (StatusBar::Mp, 68.0, Color::srgb(0.12, 0.40, 0.72)),
            ] {
                root.spawn((
                    bar,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(10),
                        top: px(top),
                        height: px(3),
                        ..default()
                    },
                    BackgroundColor(color),
                ));
            }
        });
    commands
        .spawn((
            ChatEntry,
            Button,
            GlobalZIndex(990),
            Node {
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
                padding: UiRect::horizontal(px(12)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.04, 0.045, 0.86)),
        ))
        .with_children(|button| {
            button.spawn((
                ChatEntryLabel,
                Text::new("Chat"),
                crystal_text_font(12.0),
                TextColor(Color::srgb(0.96, 0.85, 0.61)),
            ));
        });
}

fn chat_entry(
    interactions: Query<&Interaction, (With<ChatEntry>, Changed<Interaction>)>,
    shell: Res<NativeShellModel>,
    mut player: ResMut<NativePlayerUiState>,
) {
    if shell.screen == NativeShellScreen::InGame
        && !player.blocks_world_click()
        && interactions.iter().any(|i| *i == Interaction::Pressed)
    {
        player.set_chat_focused(true);
    }
}

fn decorate_belt(
    mut commands: Commands,
    targets: Query<Entity, Added<CrystalHudBeltHitTarget>>,
    backdrops: Query<Entity, Added<CrystalChatBackdrop>>,
    controls: Query<(Entity, &CrystalChatAction), Added<CrystalChatAction>>,
) {
    for entity in &targets {
        commands.entity(entity).insert((
            BackgroundColor(Color::srgba(0.025, 0.03, 0.035, 0.82)),
            BorderColor::all(Color::srgba(0.65, 0.50, 0.23, 0.86)),
        ));
    }
    for entity in &backdrops {
        commands.entity(entity).insert(BackgroundColor(Color::NONE));
    }
    for (entity, action) in &controls {
        if let Some(label) = phone_chat_label(*action) {
            commands.entity(entity).remove::<ImageNode>().insert((
                PhoneChatControlLabel,
                // Source filter/settings controls precede the frame in the
                // desktop tree. Keep phone labels above its translucent fill.
                ZIndex(1),
                Text::new(label),
                TextFont {
                    font: FontSource::SystemUi,
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.85, 0.61)),
                TextLayout::justify(Justify::Center),
                BackgroundColor(Color::srgba(0.03, 0.035, 0.04, 0.82)),
                BorderColor::all(Color::srgba(0.65, 0.50, 0.23, 0.8)),
            ));
        }
    }
}

fn phone_chat_label(action: CrystalChatAction) -> Option<&'static str> {
    Some(match action {
        CrystalChatAction::Up => "Up",
        CrystalChatAction::Down => "Down",
        CrystalChatAction::Settings => "Set",
        CrystalChatAction::FilterAll => "All",
        CrystalChatAction::FilterShout => "Shout",
        CrystalChatAction::FilterWhisper => "Whisper",
        CrystalChatAction::FilterLover => "Lover",
        CrystalChatAction::FilterMentor => "Mentor",
        CrystalChatAction::FilterGroup => "Group",
        CrystalChatAction::FilterGuild => "Guild",
        CrystalChatAction::TradeRequest => "Trade",
        CrystalChatAction::Resize => "Size",
        _ => return None,
    })
}

fn phone_filter_index(action: CrystalChatAction) -> Option<usize> {
    [
        CrystalChatAction::FilterAll,
        CrystalChatAction::FilterShout,
        CrystalChatAction::FilterWhisper,
        CrystalChatAction::FilterLover,
        CrystalChatAction::FilterMentor,
        CrystalChatAction::FilterGroup,
        CrystalChatAction::FilterGuild,
        CrystalChatAction::TradeRequest,
        CrystalChatAction::Resize,
    ]
    .iter()
    .position(|entry| *entry == action)
}

#[derive(SystemParam)]
struct PhoneNodes<'w, 's> {
    hud_roots: Query<'w, 's, (Entity, &'static Children), With<CrystalHudRoot>>,
    layers: Query<'w, 's, Entity, Or<(With<CrystalHudBeltLayer>, With<CrystalHudMiniMapLayer>)>>,
    belt_layers: Query<'w, 's, Entity, With<CrystalHudBeltLayer>>,
    belt_icons: Query<'w, 's, Entity, With<CrystalHudBeltIcon>>,
    belt_nodes: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static CrystalHudBeltHitTarget>,
            Option<&'static CrystalHudBeltKey>,
            Option<&'static CrystalHudBeltItem>,
        ),
        Or<(
            With<CrystalHudBeltHitTarget>,
            With<CrystalHudBeltKey>,
            With<CrystalHudBeltItem>,
            With<CrystalHudBeltFrame>,
            With<CrystalHudBeltTint>,
            With<CrystalBeltControlAction>,
        )>,
    >,
    chat_roots: Query<'w, 's, Entity, With<CrystalChatRoot>>,
    chat_elements: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static CrystalChatLine>,
            Option<&'static CrystalChatAction>,
            Has<CrystalChatInput>,
            Has<CrystalChatBackdrop>,
            Option<&'static ChildOf>,
        ),
        (With<CrystalChatElement>, Without<CrystalChatSettingsModal>),
    >,
    status_roots: Query<'w, 's, Entity, With<PhoneStatus>>,
    labels: Query<'w, 's, (Entity, &'static StatusLabel)>,
    bars: Query<'w, 's, (Entity, &'static StatusBar)>,
    entry: Query<'w, 's, Entity, With<ChatEntry>>,
    entry_labels: Query<'w, 's, Entity, With<ChatEntryLabel>>,
    control_labels: Query<'w, 's, Entity, With<PhoneChatControlLabel>>,
    presentation: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, &'static mut Node>,
            Query<'w, 's, &'static mut ImageNode>,
            Query<'w, 's, &'static mut TextFont>,
            Query<'w, 's, &'static mut Text>,
            Query<'w, 's, &'static mut BackgroundColor>,
        ),
    >,
}

fn fit_phone_hud(
    windows: Query<&Window>,
    scale: Res<UiScale>,
    android: Res<AndroidShellState>,
    shell: Res<NativeShellModel>,
    player: Res<NativePlayerUiState>,
    model: Res<UiReadModel>,
    chat: Res<CrystalChatState>,
    belt: Res<CrystalBeltPresentation>,
    host: Res<crate::shared_shell::HostState>,
    mut bounds: ResMut<CrystalChatPointerBounds>,
    nodes: PhoneNodes,
) {
    let PhoneNodes {
        hud_roots,
        layers,
        belt_layers,
        belt_icons,
        belt_nodes,
        chat_roots,
        chat_elements,
        status_roots,
        labels,
        bars,
        entry,
        entry_labels,
        control_labels,
        mut presentation,
    } = nodes;
    let Ok(window) = windows.single() else {
        return;
    };
    let dpi = window.scale_factor();
    let safe = &android.safe_area;
    let focused = player.chat_focused();
    let row_count = chat_elements
        .iter()
        .filter_map(|(_, row, _, _, _, _)| row.map(|r| r.row + 1))
        .max()
        .unwrap_or(0);
    let layout = phone_layout(
        Vec2::new(window.width(), window.height()),
        Vec4::new(safe.left, safe.top, safe.right, safe.bottom) / dpi,
        host.ime_bottom / dpi,
        scale.0,
        focused,
        row_count,
    );
    let u = layout.unit;
    let in_game = shell.screen == NativeShellScreen::InGame && !player.local_keys.camera_hidden;
    let playing = in_game && !player.blocks_world_click();
    // The old desktop frame, status labels and tiny menu buttons are not a
    // second set of phone controls. Keep only the real belt and minimap layers.
    let mut hud_origin = Vec2::ZERO;
    for (entity, children) in &hud_roots {
        if let Ok(root) = presentation.p0().get_mut(entity) {
            hud_origin = Vec2::new(val_px(root.left), val_px(root.top));
        }
        for child in children.iter() {
            if layers.get(child).is_err() {
                if let Ok(mut node) = presentation.p0().get_mut(child) {
                    node.display = Display::None;
                }
            }
        }
    }
    for entity in &status_roots {
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            let border = UiRect::all(px(u));
            *node = rect_node(layout.status, u);
            node.border = border;
            node.border_radius = BorderRadius::all(px(8.0 * u));
            node.display = if in_game && !player.blocks_world_click() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    for (entity, label) in &labels {
        let (top, text) = match label {
            StatusLabel::Name => (
                8.0,
                format!(
                    "{}  ·  Lv.{}",
                    model.player.name.as_deref().unwrap_or("—"),
                    model.player.level
                ),
            ),
            StatusLabel::Hp => (29.0, format!("HP  {}", model.player.hp_label())),
            StatusLabel::Mp => (51.0, format!("MP  {}", model.player.mp_label())),
            StatusLabel::Footer => (
                78.0,
                format!(
                    "Gold {}   XP {}",
                    model.player.gold,
                    model.player.experience_percent_label()
                ),
            ),
        };
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            node.left = px(10.0 * u);
            node.top = px(top * u);
            node.max_width = px((layout.status.width() - 20.0) * u);
            node.overflow = Overflow::clip();
            node.display = if matches!(label, StatusLabel::Footer) && window.height() < 320.0 {
                Display::None
            } else {
                Display::Flex
            };
        }
        if let Ok(mut font) = presentation.p2().get_mut(entity) {
            font.font_size = FontSize::Px(12.0 * u);
        }
        if let Ok(mut value) = presentation.p3().get_mut(entity) {
            if value.0 != text {
                value.0 = text;
            }
        }
    }
    for (entity, bar) in &bars {
        let (top, ratio) = match bar {
            StatusBar::Hp => (46.0, model.player.normalized_hp()),
            StatusBar::Mp => (68.0, model.player.normalized_mp()),
        };
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            node.left = px(10.0 * u);
            node.top = px(top * u);
            node.width = px((layout.status.width() - 20.0) * ratio * u);
            node.height = px(3.0 * u);
        }
    }
    // Buttons stay the shared inventory hit targets, preserving authority and
    // drag/drop ownership. Orientation is a desktop preference, not overwritten.
    for entity in &belt_layers {
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            *node = rect_node(layout.belt, u);
            node.left = px(layout.belt.min.x * u - hud_origin.x);
            node.top = px(layout.belt.min.y * u - hud_origin.y);
            node.display =
                if playing && belt.visible && !focused && layout.belt.min.y > layout.status.max.y {
                    Display::Flex
                } else {
                    Display::None
                };
        }
    }
    for (entity, target, key, count) in &belt_nodes {
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            if let Some(target) = target {
                *node = rect_node(
                    Rect::from_corners(
                        Vec2::new(f32::from(target.slot) * TAP, 0.0),
                        Vec2::new(f32::from(target.slot + 1) * TAP, TAP),
                    ),
                    u,
                );
                node.align_items = AlignItems::Center;
                node.justify_content = JustifyContent::Center;
                node.border = UiRect::all(px(u));
                node.border_radius = BorderRadius::all(px(6.0 * u));
            } else if let Some(count) = count {
                *node = rect_node(
                    Rect::from_corners(
                        Vec2::new(f32::from(count.slot) * TAP + 2.0, TAP - 14.0),
                        Vec2::new(f32::from(count.slot + 1) * TAP - 4.0, TAP),
                    ),
                    u,
                );
                node.overflow = Overflow::clip();
            } else {
                node.display = Display::None;
            }
        }
        if count.is_some() || key.is_some() {
            if let Ok(mut font) = presentation.p2().get_mut(entity) {
                font.font_size = FontSize::Px(10.0 * u);
            }
        }
    }
    for entity in &belt_icons {
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            if node.display != Display::None {
                // Shared layout refreshes these source-relative bounds every
                // Update. Magnify them without distorting the item or its
                // authored alpha-padding/true-size offsets.
                node.left = px((val_px(node.left) + 8.0) * u);
                node.top = px((val_px(node.top) + 8.0) * u);
                node.width = px(val_px(node.width) * u);
                node.height = px(val_px(node.height) * u);
            }
        }
    }
    let chat_root = chat_roots.single().ok();
    let chat_origin = chat_root
        .and_then(|entity| {
            presentation
                .p0()
                .get_mut(entity)
                .ok()
                .map(|node| Vec2::new(val_px(node.left), val_px(node.top)))
        })
        .unwrap_or_default();
    let chat_visible = in_game
        && !player.amount_modal_open()
        && !player.core.chat_settings_open()
        && (playing || focused);
    let first_row = row_count.saturating_sub(phone_history_rows(window.height(), focused));
    let row_bottom = layout.chat.max.y - TAP - 4.0;
    let filter_top = row_bottom - TAP - 4.0;
    let lines_bottom = if focused {
        filter_top - 4.0
    } else {
        row_bottom
    };
    let local_rect =
        |min: Vec2, max: Vec2| Rect::from_corners(min * u - chat_origin, max * u - chat_origin);
    for (entity, row, action, input, backdrop, parent) in &chat_elements {
        // Settings descendants retain their authored modal geometry; only the
        // direct gameplay chat children belong to this phone layout.
        if parent.is_some_and(|parent| Some(parent.parent()) != chat_root) {
            continue;
        }
        let mut target = None;
        if backdrop {
            target = Some(local_rect(layout.chat.min, layout.chat.max));
            if let Ok(mut image) = presentation.p1().get_mut(entity) {
                image.color = Color::NONE;
            }
            let alpha = if chat.applied_settings.transparent {
                0.28
            } else {
                0.72
            };
            // Added separately below: no recolouring of shared image assets.
            if let Ok(mut background) = presentation.p4().get_mut(entity) {
                background.0 = Color::srgba(0.02, 0.025, 0.03, alpha);
            }
        } else if let Some(row) = row {
            if row.row >= first_row && (!focused || !input) {
                let y = layout.chat.min.y + 4.0 + (row.row - first_row) as f32 * ROW;
                if y + ROW <= lines_bottom {
                    target = Some(local_rect(
                        Vec2::new(layout.chat.min.x + 8.0, y),
                        Vec2::new(layout.chat.max.x - 8.0, y + ROW),
                    ));
                }
            }
        } else if input {
            target = Some(local_rect(
                Vec2::new(layout.chat.min.x + 8.0, row_bottom),
                Vec2::new(layout.chat.max.x - TAP * 3.0 - 4.0, layout.chat.max.y),
            ));
        } else if let Some(action) = action {
            let index = match action {
                CrystalChatAction::Up => Some(0),
                CrystalChatAction::Down => Some(1),
                CrystalChatAction::Settings => Some(2),
                _ => None,
            };
            if let Some(index) = index {
                let x = layout.chat.max.x - TAP * (3 - index) as f32;
                target = Some(local_rect(
                    Vec2::new(x, row_bottom),
                    Vec2::new(x + TAP, layout.chat.max.y),
                ));
            } else if focused {
                if let Some(index) = phone_filter_index(*action) {
                    let step = layout.chat.width() / 9.0;
                    let x = layout.chat.min.x + index as f32 * step;
                    target = Some(local_rect(
                        Vec2::new(x, filter_top),
                        Vec2::new(x + step, row_bottom - 4.0),
                    ));
                }
            }
        }
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            if let Some(rect) = target {
                *node = rect_node(rect, 1.0);
                node.overflow = Overflow::clip();
                node.border_radius = BorderRadius::all(px(6.0 * u));
                if action.is_some() {
                    node.align_items = AlignItems::Center;
                    node.justify_content = JustifyContent::Center;
                    node.border = UiRect::all(px(u));
                    node.padding = UiRect::top(px(15.0 * u));
                }
                node.display = if chat_visible {
                    Display::Flex
                } else {
                    Display::None
                };
            } else {
                node.display = Display::None;
            }
        }
        if let Some(action) = action.filter(|action| phone_chat_label(**action).is_some()) {
            if let Ok(mut background) = presentation.p4().get_mut(entity) {
                let active = CrystalChatFilter::from_action(*action)
                    .is_some_and(|filter| filter == chat.filter);
                background.0 = if active {
                    Color::srgba(0.28, 0.20, 0.07, 0.94)
                } else {
                    Color::srgba(0.03, 0.035, 0.04, 0.82)
                };
            }
        }
        if action.is_some_and(|action| phone_chat_label(*action).is_some()) {
            if let Ok(mut image) = presentation.p1().get_mut(entity) {
                image.color = Color::NONE;
            }
        }
        if row.is_some() || input {
            if let Ok(mut font) = presentation.p2().get_mut(entity) {
                font.font_size = FontSize::Px(12.0 * u);
            }
        }
    }
    for entity in &entry {
        if let Ok(mut node) = presentation.p0().get_mut(entity) {
            *node = rect_node(
                Rect::from_corners(
                    Vec2::new(layout.chat.min.x + 4.0, row_bottom),
                    Vec2::new(layout.chat.max.x - TAP * 3.0 - 4.0, layout.chat.max.y),
                ),
                u,
            );
            node.align_items = AlignItems::Center;
            node.padding = UiRect::horizontal(px(12.0 * u));
            node.display = if chat_visible && !focused {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    for entity in &entry_labels {
        if let Ok(mut font) = presentation.p2().get_mut(entity) {
            font.font_size = FontSize::Px(12.0 * u);
        }
    }
    for entity in &control_labels {
        if let Ok(mut font) = presentation.p2().get_mut(entity) {
            if font.font_size != FontSize::Px(12.0 * u) {
                font.font_size = FontSize::Px(12.0 * u);
            }
        }
    }
    bounds.0 = Some(if chat_visible {
        Rect::from_corners(layout.chat.min * dpi, layout.chat.max * dpi)
    } else {
        Rect::EMPTY
    });
}

fn val_px(val: Val) -> f32 {
    if let Val::Px(value) = val {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "phone_hud_tests.rs"]
mod tests;
