//! Opt-in phone presentation; catalog, quantity and purchase rules stay shared.
use super::*;
use bevy::input::touch::Touches;
use bevy::ui::UiGlobalTransform;

const TAP: f32 = 48.0;
const GAP: f32 = 8.0;
const INK: Color = Color::srgb(0.91, 0.86, 0.72);
const GOLD: Color = Color::srgb(0.65, 0.48, 0.21);
const PANEL: Color = Color::srgb(0.025, 0.022, 0.015);
const BUTTON: Color = Color::srgb(0.10, 0.075, 0.035);

/// Android logical workspace and inverse authored-stage scale. Missing or
/// invalid input leaves the Windows/desktop renderer unchanged.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct PhoneGameShopPresentation {
    pub workspace: Vec2,
    pub authored_unit: f32,
}

impl PhoneGameShopPresentation {
    pub fn fit(viewport: Vec2, insets: Vec4, stage_scale: f32) -> Self {
        if !viewport.is_finite()
            || !insets.is_finite()
            || insets.min_element() < 0.0
            || !stage_scale.is_finite()
            || stage_scale <= 0.0
        {
            return Self::default();
        }
        let next = Self {
            workspace: (viewport
                - Vec2::new(insets.x + insets.z, insets.y + insets.w)
                - Vec2::splat(32.0))
            .max(Vec2::ZERO),
            authored_unit: 1.0 / stage_scale,
        };
        if next.is_valid() {
            next
        } else {
            Self::default()
        }
    }
    pub fn is_valid(&self) -> bool {
        self.workspace.is_finite()
            && self.workspace.x >= 320.0
            && self.workspace.y >= 200.0
            && self.authored_unit.is_finite()
            && self.authored_unit > 0.0
    }
}

/// Android reads actual computed controls/text for native acceptance evidence.
#[derive(Component, Debug)]
pub struct PhoneGameShopControl {
    action: Option<PhoneAction>,
    surface: Surface,
    clip: Option<ScrollKey>,
}
#[derive(Component)]
pub struct PhoneGameShopText;
#[derive(Component)]
pub struct PhoneGameShopScrollArea(ScrollKey);
#[derive(Component)]
pub(in crate::crystal_ui::overlays) struct SearchAnchor(f32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Catalog,
    Filters,
    Preview,
    Confirmation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScrollKey {
    Products,
    Filters,
    Message,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhoneAction {
    Shared(OverlayButton),
    Filters(bool),
    Scroll(ScrollKey, bool),
}

#[derive(Clone, Debug, PartialEq)]
struct Owner {
    page: usize,
    class: Option<u8>,
    section: Section,
    category: Option<String>,
    search: String,
    preview: Option<i32>,
    confirmation: Option<PurchasePrompt>,
}
impl Owner {
    fn capture(state: &NativePlayerUiState) -> Self {
        let dialog = &state.game_shop_dialog;
        Self {
            page: state.game_shop_page,
            class: dialog.class,
            section: dialog.section,
            category: dialog.category.clone(),
            search: dialog.search.clone(),
            preview: dialog.preview,
            confirmation: dialog.confirmation.clone(),
        }
    }
}
#[derive(Clone)]
struct Gesture {
    touch: Option<u64>,
    action: Option<PhoneAction>,
    scroll: Option<Entity>,
    start: Vec2,
    last: Vec2,
    dragged: bool,
    owner: Owner,
    presentation: PhoneGameShopPresentation,
    surface: Surface,
}

/// Only view/input memory. It never edits stock, prices, eligibility or saves.
#[derive(Resource, Default)]
pub(in crate::crystal_ui::overlays) struct InputState {
    pub(in crate::crystal_ui::overlays) clicks: Vec<OverlayButton>,
    pub(in crate::crystal_ui::overlays) filters_open: bool,
    owner: Option<Owner>,
    reset: Option<u64>,
    offsets: [Vec2; 3],
    gesture: Option<Gesture>,
}
impl InputState {
    fn offset(&self, key: ScrollKey) -> Vec2 {
        self.offsets[key as usize]
    }
    fn remember(&mut self, key: ScrollKey, value: Vec2) {
        self.offsets[key as usize] = value;
    }
}
fn surface(state: &NativePlayerUiState, input: &InputState) -> Surface {
    if state.game_shop_dialog.confirmation.is_some() {
        Surface::Confirmation
    } else if state.game_shop_dialog.preview.is_some() {
        Surface::Preview
    } else if input.filters_open {
        Surface::Filters
    } else {
        Surface::Catalog
    }
}

/// Track one physical owner and dispatch the SAME shared actions on release.
/// A scroll, cancellation, scene change or covered control cannot buy anything.
pub(in crate::crystal_ui::overlays) fn pointer_input(
    presentation: Option<Res<PhoneGameShopPresentation>>,
    mut input: ResMut<InputState>,
    shell: Res<NativeShellModel>,
    state: Res<NativePlayerUiState>,
    reset: Res<SessionResetRevision>,
    touches: Option<Res<Touches>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window>,
    controls: Query<(&ComputedNode, &UiGlobalTransform, &PhoneGameShopControl)>,
    mut areas: Query<(
        Entity,
        &PhoneGameShopScrollArea,
        &ComputedNode,
        &UiGlobalTransform,
        &mut ScrollPosition,
    )>,
) {
    input.clicks.clear();
    if input.reset != Some(reset.0) {
        *input = InputState {
            reset: Some(reset.0),
            ..default()
        };
    }
    let Some(phone) = presentation.as_deref().filter(|phone| phone.is_valid()) else {
        input.gesture = None;
        return;
    };
    if shell.screen != NativeShellScreen::InGame || !state.shop_open() {
        input.owner = None;
        input.filters_open = false;
        input.offsets = Default::default();
        input.gesture = None;
        return;
    }
    let Ok(window) = windows.single() else {
        input.gesture = None;
        return;
    };
    if !window.focused || state.leave_game.blocks() || state.friends.modal.is_some() {
        input.gesture = None;
        return;
    }
    let owner = Owner::capture(&state);
    let changed = input.owner.as_ref() != Some(&owner);
    if changed {
        input.owner = Some(owner.clone());
        input.gesture = None;
        input.offsets = Default::default();
    } else {
        // Do not re-import the previous tree's offsets during a page/filter
        // transition. Stable frames retain the actual layout-clamped offsets.
        for (_, area, node, _, _) in &areas {
            input.remember(area.0, node.scroll_position * node.inverse_scale_factor);
        }
    }
    let current = surface(&state, &input);
    let density = window.scale_factor();
    let hit = |point: Vec2| {
        controls.iter().find_map(|(node, transform, control)| {
            let visible = control.clip.is_none_or(|key| {
                areas.iter().any(|(_, area, clip, transform, _)| {
                    area.0 == key && clip.contains_point(*transform, point)
                })
            });
            (control.surface == current && visible && node.contains_point(*transform, point))
                .then_some(control.action)
                .flatten()
        })
    };
    let scroll_at = |point: Vec2| {
        areas.iter().find_map(|(entity, area, node, transform, _)| {
            let visible = matches!(
                (current, area.0),
                (Surface::Catalog, ScrollKey::Products)
                    | (Surface::Filters, ScrollKey::Filters)
                    | (Surface::Confirmation, ScrollKey::Message)
            );
            (visible && node.contains_point(*transform, point)).then_some(entity)
        })
    };
    if input.gesture.is_none() {
        let start = touches
            .as_deref()
            .and_then(|touches| {
                touches.iter_just_pressed().find_map(|touch| {
                    let point = touch.start_position() * density;
                    let action = hit(point);
                    let scroll = scroll_at(point);
                    (action.is_some() || scroll.is_some()).then_some((
                        Some(touch.id()),
                        point,
                        action,
                        scroll,
                    ))
                })
            })
            .or_else(|| {
                if touches
                    .as_deref()
                    .is_some_and(|t| t.iter().next().is_some())
                    || !mouse
                        .as_deref()
                        .is_some_and(|m| m.just_pressed(MouseButton::Left))
                {
                    return None;
                }
                let point = window.cursor_position()? * density;
                let action = hit(point);
                let scroll = scroll_at(point);
                (action.is_some() || scroll.is_some()).then_some((None, point, action, scroll))
            });
        if let Some((touch, point, action, scroll)) = start {
            input.gesture = Some(Gesture {
                touch,
                action,
                scroll,
                start: point,
                last: point,
                dragged: false,
                owner: owner.clone(),
                presentation: *phone,
                surface: current,
            });
        }
    }
    let Some(mut gesture) = input.gesture.clone() else {
        return;
    };
    if gesture.owner != owner || gesture.presentation != *phone || gesture.surface != current {
        input.gesture = None;
        return;
    }
    let (point, ended, cancelled) = if let Some(id) = gesture.touch {
        let Some(touches) = touches.as_deref() else {
            input.gesture = None;
            return;
        };
        if let Some(touch) = touches.get_pressed(id) {
            (touch.position() * density, false, false)
        } else if let Some(touch) = touches.iter_just_released().find(|t| t.id() == id) {
            (touch.position() * density, true, false)
        } else {
            (gesture.last, true, true)
        }
    } else {
        let pressed = mouse
            .as_deref()
            .is_some_and(|m| m.pressed(MouseButton::Left));
        let released = mouse
            .as_deref()
            .is_some_and(|m| m.just_released(MouseButton::Left));
        (
            window
                .cursor_position()
                .map(|p| p * density)
                .unwrap_or(gesture.last),
            !pressed || released,
            !pressed && !released,
        )
    };
    let released = hit(point);
    gesture.dragged |= point.distance(gesture.start) > 8.0 * density;
    if gesture.dragged && !cancelled {
        if let Some((_, area, node, _, mut offset)) =
            gesture.scroll.and_then(|id| areas.get_mut(id).ok())
        {
            let max = (node.content_size - node.size).max(Vec2::ZERO) * node.inverse_scale_factor;
            offset.0.y = (offset.0.y - (point.y - gesture.last.y) * node.inverse_scale_factor)
                .clamp(0.0, max.y);
            input.remember(area.0, offset.0);
        }
    }
    gesture.last = point;
    if ended {
        if !cancelled && !gesture.dragged {
            if let Some(action) = gesture.action.filter(|action| Some(*action) == released) {
                match action {
                    PhoneAction::Shared(action) => {
                        input.clicks.push(action);
                        if current == Surface::Filters {
                            input.filters_open = false;
                        }
                    }
                    PhoneAction::Filters(open) => input.filters_open = open,
                    PhoneAction::Scroll(key, down) => {
                        if let Some((_, area, node, _, mut offset)) =
                            areas.iter_mut().find(|(_, a, _, _, _)| a.0 == key)
                        {
                            let max = (node.content_size - node.size).max(Vec2::ZERO)
                                * node.inverse_scale_factor;
                            let step = node.size.y * node.inverse_scale_factor * 0.75;
                            offset.0.y =
                                (offset.0.y + if down { step } else { -step }).clamp(0.0, max.y);
                            input.remember(area.0, offset.0);
                        }
                    }
                }
            }
        }
        input.gesture = None;
    } else {
        input.gesture = Some(gesture);
    }
}

fn label(parent: &mut ChildSpawnerCommands, value: &str, unit: f32, heading: bool) {
    label_colored(parent, value, unit, heading, INK);
}
fn label_colored(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    unit: f32,
    heading: bool,
    color: Color,
) {
    parent.spawn((
        PhoneGameShopText,
        Node {
            min_width: Val::Px(0.0),
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            ..default()
        },
        Text::new(value),
        TextFont {
            font: FontSource::SystemUi,
            font_size: FontSize::Px(if heading { 16.0 } else { 14.0 } * unit),
            ..default()
        },
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        FocusPolicy::Pass,
    ));
}
fn control(
    parent: &mut ChildSpawnerCommands,
    value: &str,
    action: PhoneAction,
    enabled: bool,
    selected: bool,
    surface: Surface,
    clip: Option<ScrollKey>,
    unit: f32,
) {
    parent
        .spawn((
            PhoneGameShopControl {
                action: enabled.then_some(action),
                surface,
                clip,
            },
            Node {
                min_width: Val::Px(TAP * unit),
                min_height: Val::Px(TAP * unit),
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(6.0 * unit)),
                border: UiRect::all(Val::Px(unit)),
                ..default()
            },
            BackgroundColor(if selected {
                Color::srgb(0.20, 0.14, 0.05)
            } else if enabled {
                BUTTON
            } else {
                PANEL
            }),
            BorderColor::all(if enabled {
                GOLD
            } else {
                Color::srgb(0.22, 0.19, 0.13)
            }),
            FocusPolicy::Block,
        ))
        .with_children(|parent| {
            label(
                parent,
                &format!(
                    "{}{}",
                    if selected { "✓ " } else { "" },
                    crate::native_i18n::tr(value)
                ),
                unit,
                false,
            );
        });
}
fn cell(
    parent: &mut ChildSpawnerCommands,
    width: f32,
    unit: f32,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            width: if width > 0.0 {
                Val::Px(width * unit)
            } else {
                Val::Auto
            },
            min_width: Val::Px(TAP * unit),
            flex_grow: if width > 0.0 { 0.0 } else { 1.0 },
            flex_basis: if width > 0.0 { Val::Auto } else { Val::Px(0.0) },
            ..default()
        })
        .with_children(content);
}
fn row(unit: f32) -> Node {
    Node {
        width: Val::Percent(100.0),
        min_width: Val::Px(0.0),
        min_height: Val::Px(TAP * unit),
        flex_shrink: 0.0,
        column_gap: Val::Px(GAP * unit),
        align_items: AlignItems::Center,
        ..default()
    }
}
fn scroll(
    parent: &mut ChildSpawnerCommands,
    key: ScrollKey,
    input: Option<&InputState>,
    unit: f32,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            PhoneGameShopScrollArea(key),
            ScrollPosition(input.map(|i| i.offset(key)).unwrap_or_default()),
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    min_width: Val::Px(0.0),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(GAP * unit),
                    ..default()
                })
                .with_children(content);
        });
}

fn product(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    item: &GameShopEntry,
    model: &GameShopModel,
    dialog: &GameShopDialogUi,
    player: &crate::read_model::PlayerStats,
    width: f32,
    unit: f32,
) {
    let quantity = dialog.quantity(item);
    let action = |a| PhoneAction::Shared(OverlayButton::GameShopControl(a));
    parent
        .spawn((
            OverlayGameShopProduct,
            Node {
                width: Val::Px(width * unit),
                min_width: Val::Px(0.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0 * unit)),
                row_gap: Val::Px(4.0 * unit),
                border: UiRect::all(Val::Px(unit)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(GOLD),
        ))
        .with_children(|parent| {
            parent.spawn(row(unit)).with_children(|parent| {
                cell(parent, TAP, unit, |parent| {
                    if let Some(assets) = assets {
                        parent
                            .spawn(Node {
                                width: Val::Px(TAP * unit),
                                height: Val::Px(TAP * unit),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            })
                            .with_children(|parent| {
                                let mut icon = parent.spawn((
                                    Node {
                                        width: Val::Px(40.0),
                                        height: Val::Px(40.0),
                                        ..default()
                                    },
                                    UiTransform {
                                        scale: Vec2::splat(unit),
                                        ..default()
                                    },
                                    Interaction::None,
                                ));
                                if let Some(document) = crystal_item_tooltip_document_from_source(
                                    &item.item_name,
                                    u16::try_from(item.image).unwrap_or_default(),
                                    u32::from(item.count),
                                    item.tooltip_source.as_ref(),
                                    player,
                                ) {
                                    icon.insert(CrystalItemHint(document));
                                }
                                icon.with_children(|parent| {
                                    if let Ok(image) = u16::try_from(item.image) {
                                        spawn_original_item_image(parent, assets, image, 40, 40);
                                    }
                                });
                            });
                    }
                });
                cell(parent, 0.0, unit, |parent| {
                    label_colored(
                        parent,
                        &super::game_shop_friendly_name(&item.item_name),
                        unit,
                        true,
                        super::game_shop_grade_color(item),
                    )
                });
                if matches!(item.item_type, 1 | 2 | 19 | 37) {
                    cell(parent, TAP, unit, |parent| {
                        control(
                            parent,
                            "Preview",
                            action(GameShopAction::Preview(item.game_shop_index)),
                            true,
                            false,
                            Surface::Catalog,
                            Some(ScrollKey::Products),
                            unit,
                        )
                    });
                }
            });
            label(
                parent,
                &format!(
                    "{} {} · ×{}",
                    crate::native_i18n::tr("STOCK:"),
                    item.stock_label(),
                    item.count
                ),
                unit,
                false,
            );
            let mut prices = Vec::new();
            if item.can_buy_gold {
                prices.push(format!(
                    "{} {}",
                    format_number(u64::from(item.gold_price) * u64::from(quantity)),
                    crate::native_i18n::tr("Gold")
                ));
            }
            if item.can_buy_credit {
                prices.push(format!(
                    "{} {}",
                    format_number(u64::from(item.credit_price) * u64::from(quantity)),
                    crate::native_i18n::tr("Credits")
                ));
            }
            label(parent, &prices.join(" / "), unit, false);
            parent.spawn(row(unit)).with_children(|parent| {
                cell(parent, TAP, unit, |parent| {
                    control(
                        parent,
                        "−",
                        action(GameShopAction::Quantity(item.game_shop_index, false)),
                        quantity > 1,
                        false,
                        Surface::Catalog,
                        Some(ScrollKey::Products),
                        unit,
                    )
                });
                cell(parent, TAP, unit, |parent| {
                    label(parent, &quantity.to_string(), unit, false)
                });
                cell(parent, TAP, unit, |parent| {
                    control(
                        parent,
                        "+",
                        action(GameShopAction::Quantity(item.game_shop_index, true)),
                        quantity < super::quantity_limit(item),
                        false,
                        Surface::Catalog,
                        Some(ScrollKey::Products),
                        unit,
                    )
                });
                cell(parent, 0.0, unit, |parent| {
                    control(
                        parent,
                        "BUY",
                        action(GameShopAction::Buy(item.game_shop_index)),
                        model.pending_purchase.is_none()
                            && !model.purchase_unknown
                            && super::can_buy(
                                item,
                                model.payment,
                                quantity,
                                player.gold,
                                player.credit,
                            ),
                        false,
                        Surface::Catalog,
                        Some(ScrollKey::Products),
                        unit,
                    )
                });
            });
        });
}

fn filters(
    parent: &mut ChildSpawnerCommands,
    model: &GameShopModel,
    dialog: &GameShopDialogUi,
    player: &str,
    phone: PhoneGameShopPresentation,
    input: Option<&InputState>,
) {
    let unit = phone.authored_unit;
    modal(parent, phone, Surface::Filters, |parent| {
        parent.spawn(row(unit)).with_children(|parent| {
            cell(parent, 0.0, unit, |parent| {
                label(parent, &crate::native_i18n::tr("Filters"), unit, true)
            });
            cell(parent, TAP, unit, |parent| {
                control(
                    parent,
                    "×",
                    PhoneAction::Filters(false),
                    true,
                    false,
                    Surface::Filters,
                    None,
                    unit,
                )
            });
        });
        scroll(parent, ScrollKey::Filters, input, unit, |parent| {
            for indices in [[0, 1, 2], [3, 4, 5]] {
                parent.spawn(row(unit)).with_children(|parent| {
                    for i in indices {
                        cell(parent, 0.0, unit, |parent| {
                            control(
                                parent,
                                CLASSES[i],
                                PhoneAction::Shared(OverlayButton::GameShopControl(
                                    GameShopAction::Class(i as u8),
                                )),
                                true,
                                dialog.class_name(player) == CLASSES[i],
                                Surface::Filters,
                                Some(ScrollKey::Filters),
                                unit,
                            )
                        });
                    }
                });
            }
            parent.spawn(row(unit)).with_children(|parent| {
                for (section, title) in [
                    (Section::All, "ALL ITEMS"),
                    (Section::Top, "TOP"),
                    (Section::Deals, "DEALS"),
                    (Section::New, "NEW"),
                ] {
                    if section == Section::New && !dialog.new_visible {
                        continue;
                    }
                    cell(parent, 0.0, unit, |parent| {
                        control(
                            parent,
                            title,
                            PhoneAction::Shared(OverlayButton::GameShopControl(
                                GameShopAction::Section(section),
                            )),
                            true,
                            dialog.section == section,
                            Surface::Filters,
                            Some(ScrollKey::Filters),
                            unit,
                        )
                    });
                }
            });
            let categories = dialog.categories(model, player, now_ticks());
            let start = dialog
                .category_start
                .min(categories.len().saturating_sub(22));
            for (i, category) in categories.iter().enumerate().skip(start).take(22) {
                control(
                    parent,
                    category,
                    PhoneAction::Shared(OverlayButton::GameShopControl(GameShopAction::Category(
                        i,
                    ))),
                    true,
                    dialog.category.as_deref().unwrap_or("Show All") == category,
                    Surface::Filters,
                    Some(ScrollKey::Filters),
                    unit,
                );
            }
            parent.spawn(row(unit)).with_children(|parent| {
                for (title, action, enabled) in [
                    ("↑", GameShopAction::CategoriesUp, start > 0),
                    (
                        "↓",
                        GameShopAction::CategoriesDown,
                        start + 22 < categories.len(),
                    ),
                ] {
                    cell(parent, 0.0, unit, |parent| {
                        control(
                            parent,
                            title,
                            PhoneAction::Shared(OverlayButton::GameShopControl(action)),
                            enabled,
                            false,
                            Surface::Filters,
                            Some(ScrollKey::Filters),
                            unit,
                        )
                    });
                }
            });
        });
        parent.spawn(row(unit)).with_children(|parent| {
            for (title, down) in [("↑", false), ("↓", true)] {
                cell(parent, 0.0, unit, |parent| {
                    control(
                        parent,
                        title,
                        PhoneAction::Scroll(ScrollKey::Filters, down),
                        true,
                        false,
                        Surface::Filters,
                        None,
                        unit,
                    )
                });
            }
        });
    });
}
fn modal(
    parent: &mut ChildSpawnerCommands,
    phone: PhoneGameShopPresentation,
    surface: Surface,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let unit = phone.authored_unit;
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(phone.workspace.x * unit),
                height: Val::Px(phone.workspace.y * unit),
                padding: UiRect::all(Val::Px(8.0 * unit)),
                border: UiRect::all(Val::Px(unit)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(GAP * unit),
                ..default()
            },
            GlobalZIndex(
                OVERLAY_NPC_DIALOG_Z
                    + if surface == Surface::Confirmation {
                        100
                    } else {
                        50
                    },
            ),
            BackgroundColor(PANEL),
            BorderColor::all(GOLD),
            FocusPolicy::Block,
        ))
        .with_children(content);
}
fn confirmation(
    parent: &mut ChildSpawnerCommands,
    prompt: &PurchasePrompt,
    phone: PhoneGameShopPresentation,
    input: Option<&InputState>,
) {
    let unit = phone.authored_unit;
    modal(parent, phone, Surface::Confirmation, |parent| {
        label(
            parent,
            &crate::native_i18n::tr("Confirm purchase"),
            unit,
            true,
        );
        scroll(parent, ScrollKey::Message, input, unit, |parent| {
            label(parent, &crate::native_i18n::format_key("game.shop.confirm",
                "Are you sure would you like to buy {quantity} x\n{item}({count}) for {total} {currency}?",
                &[("quantity", &prompt.quantity.to_string()), ("item", &crate::player_text::name(&prompt.name)),
                  ("count", &prompt.count.to_string()), ("total", &format_number(u64::from(prompt.total))),
                  ("currency", &crate::native_i18n::tr(if prompt.payment == GameShopPaymentType::Gold { "Gold" } else { "Credits" }))]),
                unit, false);
        });
        parent.spawn(row(unit)).with_children(|parent| {
            for (title, action) in [
                ("YES", GameShopAction::Confirm),
                ("NO", GameShopAction::Cancel),
            ] {
                cell(parent, 0.0, unit, |parent| {
                    control(
                        parent,
                        title,
                        PhoneAction::Shared(OverlayButton::GameShopControl(action)),
                        true,
                        false,
                        Surface::Confirmation,
                        None,
                        unit,
                    )
                });
            }
        });
    });
}
fn preview(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    geometry: Option<&PreviewGeometry>,
    item: &GameShopEntry,
    inventory: &InventoryModel,
    ui: &UiReadModel,
    dialog: &GameShopDialogUi,
    phone: PhoneGameShopPresentation,
) {
    let unit = phone.authored_unit;
    modal(parent, phone, Surface::Preview, |parent| {
        parent.spawn(row(unit)).with_children(|parent| {
            cell(parent, 0.0, unit, |parent| {
                label(
                    parent,
                    &game_shop_friendly_name(&item.item_name),
                    unit,
                    true,
                )
            });
            cell(parent, TAP, unit, |parent| {
                control(
                    parent,
                    "×",
                    PhoneAction::Shared(OverlayButton::GameShopControl(
                        GameShopAction::PreviewClose,
                    )),
                    true,
                    false,
                    Surface::Preview,
                    None,
                    unit,
                )
            });
        });
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|parent| {
                if let (Some(assets), Some(geometry)) = (assets, geometry) {
                    for (library, index) in preview_layers(
                        item,
                        inventory,
                        ui.player.gender.as_deref(),
                        dialog.direction,
                        dialog.frame_ms,
                    ) {
                        if let Some(frame) = (geometry.0)(&library, index) {
                            if frame.width > 0.0 && frame.height > 0.0 {
                                spawn_static_overlay_sprite(
                                    parent,
                                    assets,
                                    format!("original-ui/{library}/{index}.png"),
                                    CrystalRect::new(
                                        (phone.workspace.x * 0.5 + frame.x) * unit,
                                        (100.0 + frame.y) * unit,
                                        frame.width * unit,
                                        frame.height * unit,
                                    ),
                                );
                            }
                        }
                    }
                }
            });
        parent.spawn(row(unit)).with_children(|parent| {
            for (title, right) in [("←", false), ("→", true)] {
                cell(parent, 0.0, unit, |parent| {
                    control(
                        parent,
                        title,
                        PhoneAction::Shared(OverlayButton::GameShopControl(
                            GameShopAction::PreviewTurn(right),
                        )),
                        true,
                        false,
                        Surface::Preview,
                        None,
                        unit,
                    )
                });
            }
        });
    });
}

/// Restyle ONLY the actual shared search editor inside our phone anchor.
/// Its revision, glyph ownership, composition and input bounds stay shared.
pub(in crate::crystal_ui::overlays) fn style_editor(
    mut commands: Commands,
    parents: Query<&ChildOf>,
    anchors: Query<&SearchAnchor>,
    mut texts: Query<(Entity, &ChildOf, &mut TextFont), With<friend_dialog::view::FriendEditText>>,
) {
    for (entity, parent, mut font) in &mut texts {
        let Some(unit) = parents
            .get(parent.parent())
            .ok()
            .and_then(|p| anchors.get(p.parent()).ok())
            .map(|a| a.0)
        else {
            continue;
        };
        font.font_size = FontSize::Px(14.0 * unit);
        commands.entity(entity).insert(PhoneGameShopText);
    }
}

pub(in crate::crystal_ui::overlays) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    model: &GameShopModel,
    ui: &UiReadModel,
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
    geometry: Option<&PreviewGeometry>,
    phone: PhoneGameShopPresentation,
    input: Option<&InputState>,
) {
    let unit = phone.authored_unit;
    let dialog = &state.game_shop_dialog;
    let entries = dialog.entries(
        model,
        ui.player.class_name.as_deref().unwrap_or_default(),
        now_ticks(),
    );
    let pages = native_game_shop_page_count(entries.len());
    let page = state.game_shop_page.min(pages - 1);
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0 * unit)),
                border: UiRect::all(Val::Px(unit)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(GAP * unit),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(GOLD),
            FocusPolicy::Block,
        ))
        .with_children(|parent| {
            parent.spawn(row(unit)).with_children(|parent| {
                cell(parent, 96.0, unit, |parent| {
                    label(parent, "GameShop", unit, true)
                });
                cell(parent, 80.0, unit, |parent| {
                    control(
                        parent,
                        "Filters",
                        PhoneAction::Filters(true),
                        true,
                        false,
                        Surface::Catalog,
                        None,
                        unit,
                    )
                });
                cell(parent, 0.0, unit, |parent| {
                    parent
                        .spawn((
                            SearchAnchor(unit),
                            PhoneGameShopControl {
                                action: Some(PhoneAction::Shared(OverlayButton::GameShopControl(
                                    GameShopAction::Search,
                                ))),
                                surface: Surface::Catalog,
                                clip: None,
                            },
                            Node {
                                width: Val::Percent(100.0),
                                min_width: Val::Px(TAP * unit),
                                min_height: Val::Px(TAP * unit),
                                padding: UiRect::all(Val::Px(6.0 * unit)),
                                border: UiRect::all(Val::Px(unit)),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                            BorderColor::all(GOLD),
                            FocusPolicy::Block,
                        ))
                        .with_children(|parent| {
                            if dialog.search_focused {
                                friend_dialog::view::render_editor_styled(
                                    parent,
                                    &dialog.search_input,
                                    CrystalRect::new(
                                        6.0 * unit,
                                        8.0 * unit,
                                        (phone.workspace.x
                                            - 96.0
                                            - 80.0
                                            - TAP
                                            - 4.0 * GAP
                                            - 18.0
                                            - 12.0)
                                            .max(TAP - 12.0)
                                            * unit,
                                        32.0 * unit,
                                    ),
                                    false,
                                    Color::NONE,
                                );
                            } else {
                                let text = if dialog.search.is_empty() {
                                    crate::native_i18n::tr("Search")
                                } else {
                                    dialog.search.clone()
                                };
                                label(parent, &text, unit, false);
                            }
                        });
                });
                cell(parent, TAP, unit, |parent| {
                    control(
                        parent,
                        "×",
                        PhoneAction::Shared(OverlayButton::CloseGameShop),
                        true,
                        false,
                        Surface::Catalog,
                        None,
                        unit,
                    )
                });
            });
            scroll(parent, ScrollKey::Products, input, unit, |parent| {
                let content_width = phone.workspace.x - 18.0;
                let columns = if content_width >= 480.0 { 2 } else { 1 };
                let width = (content_width - GAP * (columns - 1) as f32) / columns as f32;
                for chunk in entries
                    .iter()
                    .skip(page * 8)
                    .take(8)
                    .collect::<Vec<_>>()
                    .chunks(columns)
                {
                    parent
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(GAP * unit),
                            flex_shrink: 0.0,
                            ..default()
                        })
                        .with_children(|parent| {
                            for item in chunk {
                                product(
                                    parent, assets, item, model, dialog, &ui.player, width, unit,
                                );
                            }
                        });
                }
                if entries.is_empty() {
                    label(parent, &crate::native_i18n::tr("No items"), unit, false);
                }
            });
            parent.spawn(row(unit)).with_children(|parent| {
                for (payment, title, balance, action) in [
                    (
                        GameShopPaymentType::Gold,
                        "Gold",
                        ui.player.gold,
                        OverlayButton::GameShopPaymentGold,
                    ),
                    (
                        GameShopPaymentType::Credit,
                        "Credits",
                        ui.player.credit,
                        OverlayButton::GameShopPaymentCredit,
                    ),
                ] {
                    cell(parent, 0.0, unit, |parent| {
                        control(
                            parent,
                            &format!(
                                "{}\n{}",
                                crate::native_i18n::tr(title),
                                format_number(u64::from(balance))
                            ),
                            PhoneAction::Shared(action),
                            true,
                            payment == model.payment,
                            Surface::Catalog,
                            None,
                            unit,
                        )
                    });
                }
                cell(parent, TAP, unit, |parent| {
                    control(
                        parent,
                        "←",
                        PhoneAction::Shared(OverlayButton::GameShopPagePrev),
                        page > 0,
                        false,
                        Surface::Catalog,
                        None,
                        unit,
                    )
                });
                cell(parent, 64.0, unit, |parent| {
                    label(parent, &format!("{} / {}", page + 1, pages), unit, false)
                });
                cell(parent, TAP, unit, |parent| {
                    control(
                        parent,
                        "→",
                        PhoneAction::Shared(OverlayButton::GameShopPageNext),
                        page + 1 < pages,
                        false,
                        Surface::Catalog,
                        None,
                        unit,
                    )
                });
                for (title, down) in [("↑", false), ("↓", true)] {
                    cell(parent, TAP, unit, |parent| {
                        control(
                            parent,
                            title,
                            PhoneAction::Scroll(ScrollKey::Products, down),
                            true,
                            false,
                            Surface::Catalog,
                            None,
                            unit,
                        )
                    });
                }
            });
            if input.is_some_and(|i| i.filters_open) {
                filters(
                    parent,
                    model,
                    dialog,
                    ui.player.class_name.as_deref().unwrap_or_default(),
                    phone,
                    input,
                );
            }
            if let Some(item) = dialog
                .preview
                .and_then(|index| model.items.iter().find(|i| i.game_shop_index == index))
            {
                preview(parent, assets, geometry, item, inventory, ui, dialog, phone);
            }
            if let Some(prompt) = &dialog.confirmation {
                confirmation(parent, prompt, phone, input);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Startup, |mut commands: Commands| {
                let phone = PhoneGameShopPresentation {
                    workspace: Vec2::new(819.0, 361.0),
                    authored_unit: 768.0 / 393.0,
                };
                let model = GameShopModel {
                    items: (0..105)
                        .map(|i| GameShopEntry {
                            game_shop_index: 2000 + i,
                            item_index: 1000 + i,
                            item_name: format!("Phone product {i:03}"),
                            category: "Regression".into(),
                            stock_level: 3,
                            gold_price: 100,
                            can_buy_gold: true,
                            ..default()
                        })
                        .collect(),
                    ..default()
                };
                let mut state = NativePlayerUiState::default();
                state.game_shop_dialog.class = Some(0);
                let mut ui = UiReadModel::default();
                ui.player.gold = 777;
                commands.insert_resource(phone);
                commands.spawn(Node::default()).with_children(|parent| {
                    render(
                        parent,
                        None,
                        &model,
                        &ui,
                        &state,
                        &InventoryModel::default(),
                        None,
                        phone,
                        None,
                    );
                });
            });
        app.update();
        app
    }

    #[test]
    fn phone_catalog_has_finger_sized_shared_controls() {
        let mut app = view_app();
        let unit = app
            .world()
            .resource::<PhoneGameShopPresentation>()
            .authored_unit;
        let world = app.world_mut();
        let nodes: Vec<_> = world
            .query_filtered::<&Node, With<PhoneGameShopControl>>()
            .iter(world)
            .cloned()
            .collect();
        assert!(
            nodes.len() >= 12,
            "Phone controls must actually be generated"
        );
        for node in nodes {
            assert!(matches!(node.min_width, Val::Px(w) if w >= 48.0 * unit));
            assert!(matches!(node.min_height, Val::Px(h) if h >= 48.0 * unit));
        }
    }

    #[test]
    fn phone_catalog_has_readable_logical_text() {
        let mut app = view_app();
        let unit = app
            .world()
            .resource::<PhoneGameShopPresentation>()
            .authored_unit;
        let world = app.world_mut();
        let fonts: Vec<_> = world
            .query_filtered::<&TextFont, With<PhoneGameShopText>>()
            .iter(world)
            .cloned()
            .collect();
        assert!(!fonts.is_empty(), "Phone labels must actually be generated");
        assert!(fonts
            .iter()
            .all(|f| matches!(f.font_size, FontSize::Px(size) if size >= 14.0 * unit)));
    }

    #[test]
    fn phone_keeps_the_shared_eight_product_page_denominator() {
        let mut app = view_app();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<OverlayGameShopProduct>>()
                .iter(world)
                .count(),
            8
        );
    }

    #[test]
    fn presentation_fits_one_safe_workspace_and_rejects_invalid_inputs() {
        let scale = 393.0 / 768.0;
        let phone = PhoneGameShopPresentation::fit(
            Vec2::new(851.0, 393.0),
            Vec4::new(12.0, 8.0, 22.0, 12.0),
            scale,
        );
        assert_eq!(phone.workspace, Vec2::new(785.0, 341.0));
        assert_eq!(phone.authored_unit, 1.0 / scale);
        for (viewport, insets, scale) in [
            (Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0),
            (Vec2::new(851.0, 393.0), Vec4::ZERO, f32::NAN),
            (Vec2::new(851.0, 393.0), Vec4::splat(-1.0), scale),
            (Vec2::new(851.0, 393.0), Vec4::splat(f32::INFINITY), scale),
            (Vec2::new(f32::NAN, 393.0), Vec4::ZERO, scale),
            (Vec2::new(320.0, 180.0), Vec4::ZERO, scale),
            (
                Vec2::new(851.0, 393.0),
                Vec4::new(0.0, 0.0, 0.0, 170.0),
                scale,
            ),
        ] {
            assert!(!PhoneGameShopPresentation::fit(viewport, insets, scale).is_valid());
        }
        assert!(
            PhoneGameShopPresentation::fit(Vec2::new(568.0, 262.0), Vec4::ZERO, scale).is_valid()
        );
    }

    fn cached_app() -> App {
        let mut app = super::super::super::tests::overlay_render_test_app();
        app.insert_resource(PhoneGameShopPresentation {
            workspace: Vec2::new(819.0, 361.0),
            authored_unit: 768.0 / 393.0,
        })
        .init_resource::<InputState>();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
            state.game_shop_dialog.class = Some(0);
        }
        app.world_mut().resource_mut::<UiReadModel>().player.gold = 777;
        app.world_mut().resource_mut::<GameShopModel>().items = (0..105)
            .map(|i| GameShopEntry {
                game_shop_index: 2000 + i,
                item_index: 1000 + i,
                item_name: format!("Phone product {i:03}"),
                category: "Regression".into(),
                can_buy_gold: true,
                gold_price: 100,
                ..default()
            })
            .collect();
        app.update();
        app
    }

    #[test]
    fn actual_retained_phone_window_exposes_all_105_shared_product_ids() {
        let mut app = cached_app();
        let original = app.world().resource::<GameShopModel>().clone();
        let mut all = Vec::new();
        for page in 0..14 {
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .game_shop_page = page;
            app.update();
            let world = app.world_mut();
            let mut ids = world
                .query::<&PhoneGameShopControl>()
                .iter(world)
                .filter_map(|c| match c.action {
                    Some(PhoneAction::Shared(OverlayButton::GameShopControl(
                        GameShopAction::Buy(id),
                    ))) => Some(id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            ids.sort_unstable();
            assert_eq!(
                ids,
                (2000 + page as i32 * 8..(2000 + (page as i32 + 1) * 8).min(2105))
                    .collect::<Vec<_>>()
            );
            all.extend(ids);
        }
        assert_eq!(all, (2000..2105).collect::<Vec<_>>());
        assert_eq!(
            *app.world().resource::<GameShopModel>(),
            original,
            "Rendering cannot mutate catalog, stock or purchase state"
        );
    }

    #[test]
    fn retained_phone_controls_survive_noop_frames_and_restore_source_without_configuration() {
        let mut app = cached_app();
        let controls = |world: &mut World| {
            world
                .query_filtered::<Entity, With<PhoneGameShopControl>>()
                .iter(world)
                .collect::<Vec<_>>()
        };
        let before = controls(app.world_mut());
        app.update();
        app.update();
        assert_eq!(controls(app.world_mut()), before);
        app.world_mut().resource_mut::<InputState>().filters_open = true;
        app.update();
        let world = app.world_mut();
        assert!(world
            .query::<&PhoneGameShopControl>()
            .iter(world)
            .any(|c| c.surface == Surface::Filters));
        app.world_mut()
            .remove_resource::<PhoneGameShopPresentation>();
        app.update();
        assert!(controls(app.world_mut()).is_empty());
        let world = app.world_mut();
        let root = world
            .query_filtered::<&Node, With<OverlayGameShop>>()
            .single(world)
            .unwrap();
        assert_eq!((root.width, root.height), (Val::Px(696.0), Val::Px(476.0)));
        assert_eq!(
            world
                .query_filtered::<Entity, With<OverlayGameShopProduct>>()
                .iter(world)
                .count(),
            8
        );
    }

    #[test]
    fn filters_and_modal_render_the_existing_shared_actions_at_phone_sizes() {
        let mut app = cached_app();
        app.world_mut().resource_mut::<InputState>().filters_open = true;
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .game_shop_dialog
            .new_visible = true;
        app.update();
        let world = app.world_mut();
        let actions = world
            .query::<&PhoneGameShopControl>()
            .iter(world)
            .filter_map(|c| c.action)
            .collect::<Vec<_>>();
        for class in 0..6 {
            assert!(
                actions.contains(&PhoneAction::Shared(OverlayButton::GameShopControl(
                    GameShopAction::Class(class)
                )))
            );
        }
        for section in [Section::All, Section::Top, Section::Deals, Section::New] {
            assert!(
                actions.contains(&PhoneAction::Shared(OverlayButton::GameShopControl(
                    GameShopAction::Section(section)
                )))
            );
        }
        assert!(
            actions.contains(&PhoneAction::Shared(OverlayButton::GameShopControl(
                GameShopAction::Category(0)
            )))
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .game_shop_dialog
            .confirmation = Some(PurchasePrompt {
            index: 2000,
            name: "Authoritative product".into(),
            quantity: 1,
            count: 1,
            payment: GameShopPaymentType::Gold,
            total: 100,
        });
        app.update();
        let world = app.world_mut();
        for action in [GameShopAction::Confirm, GameShopAction::Cancel] {
            assert!(world
                .query::<&PhoneGameShopControl>()
                .iter(world)
                .any(|c| c.surface == Surface::Confirmation
                    && c.action
                        == Some(PhoneAction::Shared(OverlayButton::GameShopControl(action)))));
        }
    }

    fn pointer_fixture() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin))
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .init_resource::<NativePlayerUiState>()
            .init_resource::<SessionResetRevision>()
            .init_resource::<InputState>()
            .insert_resource(PhoneGameShopPresentation {
                workspace: Vec2::new(819.0, 361.0),
                authored_unit: 1.0,
            })
            .add_systems(Update, pointer_input);
        {
            let mut player = app.world_mut().resource_mut::<NativePlayerUiState>();
            player.core.screen = mir2_ui_core::state::UiScreen::InGame;
            player.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        }
        let window = app.world_mut().spawn(Window::default()).id();
        let control = add_control(
            app.world_mut(),
            OverlayButton::GameShopPageNext,
            Surface::Catalog,
            None,
            Vec2::new(200.0, 150.0),
        );
        app.update();
        (app, window, control)
    }
    fn add_control(
        world: &mut World,
        action: OverlayButton,
        surface: Surface,
        clip: Option<ScrollKey>,
        center: Vec2,
    ) -> Entity {
        world
            .spawn((
                PhoneGameShopControl {
                    action: Some(PhoneAction::Shared(action)),
                    surface,
                    clip,
                },
                ComputedNode {
                    size: Vec2::new(100.0, 60.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from_translation(center),
            ))
            .id()
    }
    fn add_area(world: &mut World, key: ScrollKey) -> Entity {
        world
            .spawn((
                PhoneGameShopScrollArea(key),
                ScrollPosition::default(),
                ComputedNode {
                    size: Vec2::new(400.0, 200.0),
                    content_size: Vec2::new(400.0, 800.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from_xy(200.0, 150.0),
            ))
            .id()
    }
    fn touch(
        app: &mut App,
        window: Entity,
        id: u64,
        phase: bevy::input::touch::TouchPhase,
        point: Vec2,
    ) {
        app.world_mut()
            .write_message(bevy::input::touch::TouchInput {
                window,
                id,
                phase,
                position: point,
                force: None,
            });
        app.update();
    }
    #[test]
    fn physical_owner_dispatches_one_shared_action_only_on_release() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, _) = pointer_fixture();
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 150.0));
        assert!(app.world().resource::<InputState>().clicks.is_empty());
        app.update();
        assert!(app.world().resource::<InputState>().clicks.is_empty());
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 150.0));
        assert_eq!(
            app.world().resource::<InputState>().clicks,
            vec![OverlayButton::GameShopPageNext]
        );
        app.update();
        assert!(app.world().resource::<InputState>().clicks.is_empty());
    }
    #[test]
    fn scrolling_from_a_buy_control_does_not_dispatch_purchase() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, control) = pointer_fixture();
        app.world_mut().entity_mut(control).despawn();
        add_control(
            app.world_mut(),
            OverlayButton::GameShopControl(GameShopAction::Buy(2000)),
            Surface::Catalog,
            Some(ScrollKey::Products),
            Vec2::new(200.0, 150.0),
        );
        let area = add_area(app.world_mut(), ScrollKey::Products);
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 150.0));
        touch(&mut app, window, 1, Moved, Vec2::new(200.0, 90.0));
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 80.0));
        assert!(app.world().resource::<InputState>().clicks.is_empty());
        assert!(app.world().get::<ScrollPosition>(area).unwrap().0.y > 0.0);
    }
    #[test]
    fn clipped_and_covered_controls_cannot_act() {
        use bevy::input::touch::TouchPhase::*;
        for surface in [Surface::Catalog, Surface::Filters, Surface::Confirmation] {
            let (mut app, window, control) = pointer_fixture();
            app.world_mut().entity_mut(control).despawn();
            add_control(
                app.world_mut(),
                OverlayButton::GameShopControl(GameShopAction::Buy(2000)),
                Surface::Catalog,
                Some(ScrollKey::Products),
                Vec2::new(200.0, 600.0),
            );
            add_area(app.world_mut(), ScrollKey::Products);
            if surface == Surface::Filters {
                app.world_mut().resource_mut::<InputState>().filters_open = true;
            }
            if surface == Surface::Confirmation {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .game_shop_dialog
                    .confirmation = Some(PurchasePrompt {
                    index: 2000,
                    name: "Test".into(),
                    quantity: 1,
                    count: 1,
                    payment: GameShopPaymentType::Gold,
                    total: 100,
                });
            }
            touch(&mut app, window, 1, Started, Vec2::new(200.0, 600.0));
            touch(&mut app, window, 1, Ended, Vec2::new(200.0, 600.0));
            assert!(app.world().resource::<InputState>().clicks.is_empty());
        }
    }
    #[test]
    fn cancellation_focus_scene_page_layout_and_modal_changes_retire_held_input() {
        use bevy::input::touch::TouchPhase::*;
        for guard in [
            "cancel", "focus", "scene", "page", "layout", "close", "modal",
        ] {
            let (mut app, window, _) = pointer_fixture();
            touch(&mut app, window, 1, Started, Vec2::new(200.0, 150.0));
            match guard {
                "focus" => app.world_mut().get_mut::<Window>(window).unwrap().focused = false,
                "scene" => app.world_mut().resource_mut::<SessionResetRevision>().0 += 1,
                "page" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .game_shop_page = 1
                }
                "layout" => {
                    app.world_mut()
                        .resource_mut::<PhoneGameShopPresentation>()
                        .workspace
                        .x -= 1.0
                }
                "close" => {
                    app.world_mut()
                        .resource_mut::<NativePlayerUiState>()
                        .core
                        .panel = mir2_ui_core::state::UiPanel::None
                }
                "modal" => app.world_mut().resource_mut::<InputState>().filters_open = true,
                _ => {}
            }
            touch(
                &mut app,
                window,
                1,
                if guard == "cancel" { Canceled } else { Ended },
                Vec2::new(200.0, 150.0),
            );
            assert!(
                app.world().resource::<InputState>().clicks.is_empty(),
                "{guard}"
            );
            assert!(
                app.world().resource::<InputState>().gesture.is_none(),
                "{guard}"
            );
        }
    }
    #[test]
    fn second_finger_cannot_release_the_first_fingers_control() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, _) = pointer_fixture();
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 150.0));
        touch(&mut app, window, 2, Started, Vec2::new(200.0, 150.0));
        touch(&mut app, window, 2, Ended, Vec2::new(200.0, 150.0));
        assert!(app.world().resource::<InputState>().clicks.is_empty());
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 150.0));
        assert_eq!(
            app.world().resource::<InputState>().clicks,
            vec![OverlayButton::GameShopPageNext]
        );
    }
    #[test]
    fn preview_animation_can_rebuild_a_control_without_replaying_or_losing_the_release() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, control) = pointer_fixture();
        app.world_mut().entity_mut(control).despawn();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .game_shop_dialog
            .preview = Some(2000);
        let action = OverlayButton::GameShopControl(GameShopAction::PreviewTurn(true));
        let control = add_control(
            app.world_mut(),
            action,
            Surface::Preview,
            None,
            Vec2::new(200.0, 150.0),
        );
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 150.0));
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .game_shop_dialog
            .frame_ms = 150;
        app.world_mut().entity_mut(control).despawn();
        add_control(
            app.world_mut(),
            action,
            Surface::Preview,
            None,
            Vec2::new(200.0, 150.0),
        );
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 150.0));
        assert_eq!(app.world().resource::<InputState>().clicks, vec![action]);
    }

    #[test]
    fn phone_release_uses_shared_buy_validation_and_never_grants_an_item() {
        use bevy::input::touch::TouchPhase::*;
        let (mut app, window, control) = pointer_fixture();
        app.world_mut().entity_mut(control).despawn();
        super::super::super::tests::init_overlay_button_test_resources(&mut app);
        app.init_resource::<UiEffectQueue>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MailComposeUi>()
            .init_resource::<ShopModel>()
            .init_resource::<GameShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>()
            .init_resource::<UiReadModel>()
            .init_resource::<PendingOperations>();
        app.add_systems(
            Update,
            super::super::super::process_overlay_buttons.after(pointer_input),
        );
        app.world_mut().resource_mut::<UiReadModel>().player.gold = 777;
        app.world_mut().resource_mut::<GameShopModel>().items = vec![GameShopEntry {
            game_shop_index: 2000,
            item_index: 1000,
            item_name: "Authoritative test".into(),
            can_buy_gold: true,
            gold_price: 100,
            stock: 3,
            stock_level: 3,
            ..default()
        }];
        add_control(
            app.world_mut(),
            OverlayButton::GameShopControl(GameShopAction::Buy(2000)),
            Surface::Catalog,
            None,
            Vec2::new(200.0, 150.0),
        );
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 150.0));
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .game_shop_dialog
            .confirmation
            .is_none());
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 150.0));
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .game_shop_dialog
            .confirmation
            .is_some());
        assert!(app
            .world()
            .resource::<GameShopModel>()
            .pending_purchase
            .is_none());
        assert!(app.world().resource::<InventoryModel>().items.is_empty());
        add_control(
            app.world_mut(),
            OverlayButton::GameShopControl(GameShopAction::Confirm),
            Surface::Confirmation,
            None,
            Vec2::new(200.0, 150.0),
        );
        touch(&mut app, window, 1, Started, Vec2::new(200.0, 150.0));
        app.world_mut().resource_mut::<GameShopModel>().items[0].stock_level = 0;
        touch(&mut app, window, 1, Ended, Vec2::new(200.0, 150.0));
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .game_shop_dialog
            .confirmation
            .is_none());
        assert!(app
            .world()
            .resource::<GameShopModel>()
            .pending_purchase
            .is_none());
        assert!(app.world().resource::<InventoryModel>().items.is_empty());
        assert_eq!(app.world().resource::<UiReadModel>().player.gold, 777);
    }
}
